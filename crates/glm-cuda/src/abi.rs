use std::fmt;

pub const ABI_VERSION: u32 = 1;
pub const HIDDEN: u32 = 6144;
pub const LOCAL_GATE_UP: u32 = 1024;
pub const LOCAL_INTERMEDIATE: u32 = 512;
pub const EXPERTS: u32 = 256;
pub const TOP_K: u32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KernelPath {
    DecodePersistent = 1,
    PrefillGrouped = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchGeometry {
    pub rows: u32,
    pub assignments: u32,
    pub path: KernelPath,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
pub struct Fc1Descriptor {
    pub abi_version: u32,
    pub struct_bytes: u32,
    pub flags: u32,
    pub path: u32,
    pub rows: u32,
    pub assignments: u32,
    pub hidden: u32,
    pub local_gate_up: u32,
    pub local_intermediate: u32,
    pub experts: u32,
    pub top_k: u32,
    pub reserved0: u32,
    pub input_bf16: u64,
    pub expert_value_base: u64,
    pub expert_scale_base: u64,
    pub expert_global_scales: u64,
    pub route_experts_u16: u64,
    pub route_tokens_u32: u64,
    pub route_slots_u8: u64,
    pub route_weights_f32: u64,
    pub expert_offsets_u32: u64,
    pub compacted_input_bf16: u64,
    pub activation_values: u64,
    pub activation_scales: u64,
    pub activation_global_scales: u64,
    pub gate_up_accum_f32: u64,
    pub output_bf16: u64,
    pub workspace_bytes: u64,
    pub sequence: u64,
    pub reserved: [u64; 4],
}

impl Fc1Descriptor {
    #[must_use]
    pub fn new(geometry: LaunchGeometry) -> Self {
        Self {
            abi_version: ABI_VERSION,
            struct_bytes: std::mem::size_of::<Self>() as u32,
            flags: 0,
            path: geometry.path as u32,
            rows: geometry.rows,
            assignments: geometry.assignments,
            hidden: HIDDEN,
            local_gate_up: LOCAL_GATE_UP,
            local_intermediate: LOCAL_INTERMEDIATE,
            experts: EXPERTS,
            top_k: TOP_K,
            reserved0: 0,
            input_bf16: 0,
            expert_value_base: 0,
            expert_scale_base: 0,
            expert_global_scales: 0,
            route_experts_u16: 0,
            route_tokens_u32: 0,
            route_slots_u8: 0,
            route_weights_f32: 0,
            expert_offsets_u32: 0,
            compacted_input_bf16: 0,
            activation_values: 0,
            activation_scales: 0,
            activation_global_scales: 0,
            gate_up_accum_f32: 0,
            output_bf16: 0,
            workspace_bytes: 0,
            sequence: 0,
            reserved: [0; 4],
        }
    }
}

pub fn validate_descriptor(descriptor: &Fc1Descriptor) -> Result<(), KernelError> {
    if descriptor.abi_version != ABI_VERSION
        || descriptor.struct_bytes as usize != std::mem::size_of::<Fc1Descriptor>()
        || descriptor.hidden != HIDDEN
        || descriptor.local_gate_up != LOCAL_GATE_UP
        || descriptor.local_intermediate != LOCAL_INTERMEDIATE
        || descriptor.experts != EXPERTS
        || descriptor.top_k != TOP_K
        || descriptor.flags != 0
        || descriptor.reserved0 != 0
        || descriptor.reserved.iter().any(|&value| value != 0)
    {
        return Err(KernelError::Abi);
    }
    let path = match descriptor.path {
        1 => KernelPath::DecodePersistent,
        2 => KernelPath::PrefillGrouped,
        _ => return Err(KernelError::Path),
    };
    let max_rows = match path {
        KernelPath::DecodePersistent => 128,
        KernelPath::PrefillGrouped => 65_536,
    };
    if descriptor.rows == 0
        || descriptor.rows > max_rows
        || descriptor.assignments == 0
        || descriptor.assignments > 65_535
        || descriptor.assignments
            > descriptor
                .rows
                .checked_mul(TOP_K)
                .ok_or(KernelError::Overflow)?
    {
        return Err(KernelError::Shape);
    }
    let required = required_pointers(descriptor);
    if required.iter().any(|&&pointer| pointer == 0) {
        return Err(KernelError::Null);
    }
    if !descriptor.expert_value_base.is_multiple_of(256)
        || !descriptor.expert_scale_base.is_multiple_of(256)
        || !descriptor.activation_values.is_multiple_of(16)
        || !descriptor.activation_scales.is_multiple_of(16)
    {
        return Err(KernelError::Alignment);
    }
    let required_workspace = workspace_bytes(descriptor.assignments)?;
    if descriptor.workspace_bytes < required_workspace {
        return Err(KernelError::Workspace {
            required: required_workspace,
            provided: descriptor.workspace_bytes,
        });
    }
    Ok(())
}

fn required_pointers(descriptor: &Fc1Descriptor) -> [&u64; 15] {
    [
        &descriptor.input_bf16,
        &descriptor.expert_value_base,
        &descriptor.expert_scale_base,
        &descriptor.expert_global_scales,
        &descriptor.route_experts_u16,
        &descriptor.route_tokens_u32,
        &descriptor.route_slots_u8,
        &descriptor.route_weights_f32,
        &descriptor.expert_offsets_u32,
        &descriptor.compacted_input_bf16,
        &descriptor.activation_values,
        &descriptor.activation_scales,
        &descriptor.activation_global_scales,
        &descriptor.gate_up_accum_f32,
        &descriptor.output_bf16,
    ]
}

pub fn workspace_bytes(assignments: u32) -> Result<u64, KernelError> {
    let assignments = u64::from(assignments);
    let padded_assignments = assignments
        .checked_add(127)
        .map(|value| value / 128 * 128)
        .ok_or(KernelError::Overflow)?;
    let compacted = assignments.checked_mul(u64::from(HIDDEN) * 2);
    let activation_values = padded_assignments.checked_mul(u64::from(HIDDEN) / 2);
    let activation_scales = padded_assignments.checked_mul(u64::from(HIDDEN) / 16);
    let activation_globals = assignments.checked_mul(4);
    let gate_up = assignments.checked_mul(u64::from(LOCAL_GATE_UP) * 4);
    let offsets = u64::from(EXPERTS + 1).checked_mul(4);
    [
        compacted,
        activation_values,
        activation_scales,
        activation_globals,
        gate_up,
        offsets,
    ]
    .into_iter()
    .try_fold(0_u64, |sum, value| {
        sum.checked_add(value.ok_or(KernelError::Overflow)?)
            .ok_or(KernelError::Overflow)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelError {
    Abi,
    Path,
    Shape,
    Null,
    Alignment,
    Overflow,
    Workspace { required: u64, provided: u64 },
    Driver(i32),
    Async(i32),
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for KernelError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_layout_is_frozen() {
        assert_eq!(std::mem::size_of::<Fc1Descriptor>(), 224);
        assert_eq!(std::mem::align_of::<Fc1Descriptor>(), 16);
    }

    #[test]
    fn every_required_decode_m_validates() {
        for rows in [1, 2, 4, 8, 16, 32, 64, 128] {
            let assignments = rows * TOP_K;
            let mut descriptor = Fc1Descriptor::new(LaunchGeometry {
                rows,
                assignments,
                path: KernelPath::DecodePersistent,
            });
            let mut pointer = 0x1000_u64;
            for field in [
                &mut descriptor.input_bf16,
                &mut descriptor.expert_value_base,
                &mut descriptor.expert_scale_base,
                &mut descriptor.expert_global_scales,
                &mut descriptor.route_experts_u16,
                &mut descriptor.route_tokens_u32,
                &mut descriptor.route_slots_u8,
                &mut descriptor.route_weights_f32,
                &mut descriptor.expert_offsets_u32,
                &mut descriptor.compacted_input_bf16,
                &mut descriptor.activation_values,
                &mut descriptor.activation_scales,
                &mut descriptor.activation_global_scales,
                &mut descriptor.gate_up_accum_f32,
                &mut descriptor.output_bf16,
            ] {
                *field = pointer;
                pointer += 0x100;
            }
            descriptor.workspace_bytes = workspace_bytes(assignments).unwrap();
            validate_descriptor(&descriptor).unwrap();
        }
    }
}
