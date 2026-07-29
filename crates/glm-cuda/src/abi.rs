use std::fmt;

pub const ABI_VERSION: u32 = 1;
pub const HIDDEN: u32 = 6144;
pub const LOCAL_GATE_UP: u32 = 1024;
pub const LOCAL_INTERMEDIATE: u32 = 512;
pub const EXPERTS: u32 = 256;
pub const TOP_K: u32 = 8;
pub const SFA_BYTES_PER_PADDED_ROW: u64 = HIDDEN as u64 / 16;
pub const EXL3_ABI_VERSION: u32 = 1;
pub const EXL3_BITS: u32 = 3;
pub const EXL3_MAX_ROWS: u32 = 3_072;
pub const EXL3_KERNEL_ABI: &str = "glmaxx.sm120.exl3.source_projection.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KernelPath {
    DecodePersistent = 1,
    PrefillGrouped = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Exl3KernelProjection {
    Gate = 1,
    Up = 2,
    Down = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchGeometry {
    pub rows: u32,
    pub assignments: u32,
    pub path: KernelPath,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupedSfaPlan {
    pub expert_byte_offsets: [u64; EXPERTS as usize + 1],
    pub total_bytes: u64,
    pub active_experts: u16,
}

pub fn grouped_sfa_plan(
    expert_assignment_offsets: &[u32; EXPERTS as usize + 1],
) -> Result<GroupedSfaPlan, KernelError> {
    if expert_assignment_offsets[0] != 0 || expert_assignment_offsets[EXPERTS as usize] > 65_535 {
        return Err(KernelError::Shape);
    }
    let mut expert_byte_offsets = [0_u64; EXPERTS as usize + 1];
    let mut active_experts = 0_u16;
    for expert in 0..EXPERTS as usize {
        let begin = expert_assignment_offsets[expert];
        let end = expert_assignment_offsets[expert + 1];
        let assignments = end.checked_sub(begin).ok_or(KernelError::Shape)?;
        let padded = u64::from(assignments)
            .checked_add(127)
            .map(|value| value / 128 * 128)
            .ok_or(KernelError::Overflow)?;
        let bytes = padded
            .checked_mul(SFA_BYTES_PER_PADDED_ROW)
            .ok_or(KernelError::Overflow)?;
        expert_byte_offsets[expert + 1] = expert_byte_offsets[expert]
            .checked_add(bytes)
            .ok_or(KernelError::Overflow)?;
        if assignments != 0 {
            active_experts = active_experts.checked_add(1).ok_or(KernelError::Overflow)?;
        }
    }
    Ok(GroupedSfaPlan {
        total_bytes: expert_byte_offsets[EXPERTS as usize],
        expert_byte_offsets,
        active_experts,
    })
}

pub fn grouped_sfa_capacity_bytes(assignments: u32) -> Result<u64, KernelError> {
    if assignments == 0 || assignments > 65_535 {
        return Err(KernelError::Shape);
    }
    let assignments = u64::from(assignments);
    let active_experts = assignments.min(u64::from(EXPERTS));
    assignments
        .checked_add(
            active_experts
                .checked_mul(127)
                .ok_or(KernelError::Overflow)?,
        )
        .and_then(|rows| rows.checked_mul(SFA_BYTES_PER_PADDED_ROW))
        .ok_or(KernelError::Overflow)
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

#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
pub struct Fc2Descriptor {
    pub abi_version: u32,
    pub struct_bytes: u32,
    pub flags: u32,
    pub path: u32,
    pub rows: u32,
    pub assignments: u32,
    pub hidden: u32,
    pub local_intermediate: u32,
    pub experts: u32,
    pub top_k: u32,
    pub reserved0: u32,
    pub reserved1: u32,
    pub input_bf16: u64,
    pub expert_value_base: u64,
    pub expert_scale_base: u64,
    pub expert_global_scales: u64,
    pub route_experts_u16: u64,
    pub route_tokens_u32: u64,
    pub route_slots_u8: u64,
    pub route_weights_f32: u64,
    pub expert_offsets_u32: u64,
    pub activation_values: u64,
    pub activation_scales: u64,
    pub activation_global_scales: u64,
    pub assignment_down_f32: u64,
    pub token_output_f32: u64,
    pub slot_assignment_u32: u64,
    pub validation_error_u32: u64,
    pub workspace_bytes: u64,
    pub sequence: u64,
    pub reserved: [u64; 4],
}

/// Direct source-order EXL3 projection correctness boundary.
///
/// The three work/output planes are FP16. `trellis_u16`, `suh_f16`, and
/// `svh_f16` point directly into the native container planes; a launch may
/// reconstruct only the scalar/tile values it immediately consumes.
#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
pub struct Exl3Descriptor {
    pub abi_version: u32,
    pub struct_bytes: u32,
    pub flags: u32,
    pub projection: u32,
    pub rows: u32,
    pub logical_k: u32,
    pub logical_n: u32,
    pub bits: u32,
    pub input_f16: u64,
    pub trellis_u16: u64,
    pub suh_f16: u64,
    pub svh_f16: u64,
    pub rotated_input_f16: u64,
    pub projected_f16: u64,
    pub output_f16: u64,
    pub validation_error_u32: u64,
    pub workspace_bytes: u64,
    pub sequence: u64,
    pub reserved: [u64; 4],
}

impl Exl3Descriptor {
    #[must_use]
    pub fn new(rows: u32, projection: Exl3KernelProjection) -> Self {
        let (logical_k, logical_n) = match projection {
            Exl3KernelProjection::Gate | Exl3KernelProjection::Up => (HIDDEN, LOCAL_INTERMEDIATE),
            Exl3KernelProjection::Down => (LOCAL_INTERMEDIATE, HIDDEN),
        };
        Self {
            abi_version: EXL3_ABI_VERSION,
            struct_bytes: std::mem::size_of::<Self>() as u32,
            flags: 0,
            projection: projection as u32,
            rows,
            logical_k,
            logical_n,
            bits: EXL3_BITS,
            input_f16: 0,
            trellis_u16: 0,
            suh_f16: 0,
            svh_f16: 0,
            rotated_input_f16: 0,
            projected_f16: 0,
            output_f16: 0,
            validation_error_u32: 0,
            workspace_bytes: 0,
            sequence: 0,
            reserved: [0; 4],
        }
    }
}

impl Fc2Descriptor {
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
            local_intermediate: LOCAL_INTERMEDIATE,
            experts: EXPERTS,
            top_k: TOP_K,
            reserved0: 0,
            reserved1: 0,
            input_bf16: 0,
            expert_value_base: 0,
            expert_scale_base: 0,
            expert_global_scales: 0,
            route_experts_u16: 0,
            route_tokens_u32: 0,
            route_slots_u8: 0,
            route_weights_f32: 0,
            expert_offsets_u32: 0,
            activation_values: 0,
            activation_scales: 0,
            activation_global_scales: 0,
            assignment_down_f32: 0,
            token_output_f32: 0,
            slot_assignment_u32: 0,
            validation_error_u32: 0,
            workspace_bytes: 0,
            sequence: 0,
            reserved: [0; 4],
        }
    }
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

pub fn validate_fc2_descriptor(descriptor: &Fc2Descriptor) -> Result<(), KernelError> {
    if descriptor.abi_version != ABI_VERSION
        || descriptor.struct_bytes as usize != std::mem::size_of::<Fc2Descriptor>()
        || descriptor.hidden != HIDDEN
        || descriptor.local_intermediate != LOCAL_INTERMEDIATE
        || descriptor.experts != EXPERTS
        || descriptor.top_k != TOP_K
        || descriptor.flags != 0
        || descriptor.reserved0 != 0
        || descriptor.reserved1 != 0
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
    let required = [
        descriptor.input_bf16,
        descriptor.expert_value_base,
        descriptor.expert_scale_base,
        descriptor.expert_global_scales,
        descriptor.route_experts_u16,
        descriptor.route_tokens_u32,
        descriptor.route_slots_u8,
        descriptor.route_weights_f32,
        descriptor.expert_offsets_u32,
        descriptor.activation_values,
        descriptor.activation_scales,
        descriptor.activation_global_scales,
        descriptor.assignment_down_f32,
        descriptor.token_output_f32,
        descriptor.slot_assignment_u32,
        descriptor.validation_error_u32,
    ];
    if required.contains(&0) {
        return Err(KernelError::Null);
    }
    if !descriptor.expert_value_base.is_multiple_of(256)
        || !descriptor.expert_scale_base.is_multiple_of(256)
        || !descriptor.activation_values.is_multiple_of(16)
        || !descriptor.activation_scales.is_multiple_of(16)
        || !descriptor.assignment_down_f32.is_multiple_of(4)
        || !descriptor.token_output_f32.is_multiple_of(4)
        || !descriptor.slot_assignment_u32.is_multiple_of(4)
        || !descriptor.validation_error_u32.is_multiple_of(4)
    {
        return Err(KernelError::Alignment);
    }
    let required_workspace = fc2_workspace_bytes(descriptor.rows, descriptor.assignments)?;
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

pub fn grouped_workspace_bytes(assignments: u32) -> Result<u64, KernelError> {
    let assignments_u64 = u64::from(assignments);
    let padded_assignments = assignments_u64
        .checked_add(127)
        .map(|value| value / 128 * 128)
        .ok_or(KernelError::Overflow)?;
    let global_sfa_bytes = padded_assignments
        .checked_mul(u64::from(HIDDEN) / 16)
        .ok_or(KernelError::Overflow)?;
    workspace_bytes(assignments)?
        .checked_sub(global_sfa_bytes)
        .and_then(|base| base.checked_add(grouped_sfa_capacity_bytes(assignments).ok()?))
        .ok_or(KernelError::Overflow)
}

pub fn fc2_workspace_bytes(rows: u32, assignments: u32) -> Result<u64, KernelError> {
    if rows == 0
        || assignments == 0
        || assignments > rows.checked_mul(TOP_K).ok_or(KernelError::Overflow)?
    {
        return Err(KernelError::Shape);
    }
    let rows = u64::from(rows);
    let assignments = u64::from(assignments);
    let padded_assignments = assignments
        .checked_add(127)
        .map(|value| value / 128 * 128)
        .ok_or(KernelError::Overflow)?;
    let activation_values = padded_assignments.checked_mul(u64::from(LOCAL_INTERMEDIATE) / 2);
    let activation_scales = padded_assignments.checked_mul(u64::from(LOCAL_INTERMEDIATE) / 16);
    let activation_globals = assignments.checked_mul(4);
    let assignment_down = assignments.checked_mul(u64::from(HIDDEN) * 4);
    let materialized_down_bf16 = assignments.checked_mul(u64::from(HIDDEN) * 2);
    let token_output = rows.checked_mul(u64::from(HIDDEN) * 4);
    let slot_assignment = rows.checked_mul(u64::from(TOP_K) * 4);
    [
        activation_values,
        activation_scales,
        activation_globals,
        assignment_down,
        materialized_down_bf16,
        token_output,
        slot_assignment,
        Some(4),
    ]
    .into_iter()
    .try_fold(0_u64, |sum, value| {
        sum.checked_add(value.ok_or(KernelError::Overflow)?)
            .ok_or(KernelError::Overflow)
    })
}

pub fn fc2_grouped_sfa_capacity_bytes(assignments: u32) -> Result<u64, KernelError> {
    if assignments == 0 || assignments > 65_535 {
        return Err(KernelError::Shape);
    }
    let assignments = u64::from(assignments);
    let active_experts = assignments.min(u64::from(EXPERTS));
    assignments
        .checked_add(
            active_experts
                .checked_mul(127)
                .ok_or(KernelError::Overflow)?,
        )
        .and_then(|rows| rows.checked_mul(u64::from(LOCAL_INTERMEDIATE) / 16))
        .ok_or(KernelError::Overflow)
}

pub fn fc2_grouped_workspace_bytes(rows: u32, assignments: u32) -> Result<u64, KernelError> {
    let global_sfa_bytes = u64::from(assignments)
        .checked_add(127)
        .map(|value| value / 128 * 128)
        .and_then(|padded| padded.checked_mul(u64::from(LOCAL_INTERMEDIATE) / 16))
        .ok_or(KernelError::Overflow)?;
    fc2_workspace_bytes(rows, assignments)?
        .checked_sub(global_sfa_bytes)
        .and_then(|base| base.checked_add(fc2_grouped_sfa_capacity_bytes(assignments).ok()?))
        .ok_or(KernelError::Overflow)
}

pub fn exl3_trellis_bytes(logical_k: u32, logical_n: u32, bits: u32) -> Result<u64, KernelError> {
    if logical_k == 0
        || logical_n == 0
        || !logical_k.is_multiple_of(16)
        || !logical_n.is_multiple_of(16)
        || bits != EXL3_BITS
    {
        return Err(KernelError::Shape);
    }
    u64::from(logical_k / 16)
        .checked_mul(u64::from(logical_n / 16))
        .and_then(|tiles| tiles.checked_mul(u64::from(16 * bits)))
        .and_then(|halves| halves.checked_mul(2))
        .ok_or(KernelError::Overflow)
}

pub fn exl3_workspace_bytes(rows: u32, logical_k: u32, logical_n: u32) -> Result<u64, KernelError> {
    if rows == 0
        || rows > EXL3_MAX_ROWS
        || !matches!(
            (logical_k, logical_n),
            (HIDDEN, LOCAL_INTERMEDIATE) | (LOCAL_INTERMEDIATE, HIDDEN)
        )
    {
        return Err(KernelError::Shape);
    }
    u64::from(rows)
        .checked_mul(
            u64::from(logical_k)
                .checked_add(u64::from(logical_n))
                .ok_or(KernelError::Overflow)?,
        )
        .and_then(|elements| elements.checked_mul(2))
        .ok_or(KernelError::Overflow)
}

pub fn validate_exl3_descriptor(descriptor: &Exl3Descriptor) -> Result<(), KernelError> {
    if descriptor.abi_version != EXL3_ABI_VERSION
        || descriptor.struct_bytes as usize != std::mem::size_of::<Exl3Descriptor>()
        || descriptor.flags != 0
        || descriptor.bits != EXL3_BITS
        || descriptor.reserved.iter().any(|&value| value != 0)
    {
        return Err(KernelError::Abi);
    }
    let expected_shape = match descriptor.projection {
        1 | 2 => (HIDDEN, LOCAL_INTERMEDIATE),
        3 => (LOCAL_INTERMEDIATE, HIDDEN),
        _ => return Err(KernelError::Path),
    };
    if (descriptor.logical_k, descriptor.logical_n) != expected_shape
        || descriptor.rows == 0
        || descriptor.rows > EXL3_MAX_ROWS
    {
        return Err(KernelError::Shape);
    }
    if [
        descriptor.input_f16,
        descriptor.trellis_u16,
        descriptor.suh_f16,
        descriptor.svh_f16,
        descriptor.rotated_input_f16,
        descriptor.projected_f16,
        descriptor.output_f16,
        descriptor.validation_error_u32,
    ]
    .contains(&0)
    {
        return Err(KernelError::Null);
    }
    if !descriptor.input_f16.is_multiple_of(2)
        || !descriptor.trellis_u16.is_multiple_of(4)
        || !descriptor.suh_f16.is_multiple_of(2)
        || !descriptor.svh_f16.is_multiple_of(2)
        || !descriptor.rotated_input_f16.is_multiple_of(2)
        || !descriptor.projected_f16.is_multiple_of(2)
        || !descriptor.output_f16.is_multiple_of(2)
        || !descriptor.validation_error_u32.is_multiple_of(4)
    {
        return Err(KernelError::Alignment);
    }
    let required =
        exl3_workspace_bytes(descriptor.rows, descriptor.logical_k, descriptor.logical_n)?;
    if descriptor.workspace_bytes < required {
        return Err(KernelError::Workspace {
            required,
            provided: descriptor.workspace_bytes,
        });
    }
    let _ = exl3_trellis_bytes(descriptor.logical_k, descriptor.logical_n, descriptor.bits)?;
    Ok(())
}

#[cfg(any(feature = "cuda-ffi", test))]
pub(crate) fn active_experts_for_grouped(
    route_experts: &[u16],
    expert_offsets: &[u32; EXPERTS as usize + 1],
) -> Result<Vec<u16>, KernelError> {
    if route_experts.is_empty() || expert_offsets[EXPERTS as usize] as usize != route_experts.len()
    {
        return Err(KernelError::Shape);
    }
    let mut active_experts = Vec::new();
    for expert in 0_u16..EXPERTS as u16 {
        let begin = expert_offsets[usize::from(expert)] as usize;
        let end = expert_offsets[usize::from(expert) + 1] as usize;
        if begin == end {
            continue;
        }
        if route_experts
            .get(begin..end)
            .is_none_or(|routes| routes.iter().any(|&candidate| candidate != expert))
        {
            return Err(KernelError::Shape);
        }
        active_experts.push(expert);
    }
    if active_experts.is_empty() {
        Err(KernelError::Shape)
    } else {
        Ok(active_experts)
    }
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
    DeviceValidation(u32),
    Topology,
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
        assert_eq!(std::mem::size_of::<Fc2Descriptor>(), 224);
        assert_eq!(std::mem::align_of::<Fc2Descriptor>(), 16);
        assert_eq!(std::mem::size_of::<Exl3Descriptor>(), 144);
        assert_eq!(std::mem::align_of::<Exl3Descriptor>(), 16);
    }

    #[test]
    fn exl3_actual_projection_descriptors_validate() {
        for projection in [
            Exl3KernelProjection::Gate,
            Exl3KernelProjection::Up,
            Exl3KernelProjection::Down,
        ] {
            for rows in [1, 2, 8, 128, 256, EXL3_MAX_ROWS] {
                let mut descriptor = Exl3Descriptor::new(rows, projection);
                let mut pointer = 0x1000_u64;
                for field in [
                    &mut descriptor.input_f16,
                    &mut descriptor.trellis_u16,
                    &mut descriptor.suh_f16,
                    &mut descriptor.svh_f16,
                    &mut descriptor.rotated_input_f16,
                    &mut descriptor.projected_f16,
                    &mut descriptor.output_f16,
                    &mut descriptor.validation_error_u32,
                ] {
                    *field = pointer;
                    pointer += 0x100;
                }
                descriptor.workspace_bytes =
                    exl3_workspace_bytes(rows, descriptor.logical_k, descriptor.logical_n).unwrap();
                validate_exl3_descriptor(&descriptor).unwrap();
            }
        }
        assert_eq!(
            exl3_trellis_bytes(HIDDEN, LOCAL_INTERMEDIATE, EXL3_BITS).unwrap(),
            1_179_648
        );
        assert_eq!(
            exl3_workspace_bytes(1, HIDDEN, LOCAL_INTERMEDIATE).unwrap(),
            13_312
        );
        assert_eq!(
            exl3_workspace_bytes(1, HIDDEN, HIDDEN),
            Err(KernelError::Shape)
        );
    }

    #[test]
    fn exl3_descriptor_rejects_shape_pointer_and_workspace_lies() {
        let mut descriptor = Exl3Descriptor::new(1, Exl3KernelProjection::Gate);
        let mut pointer = 0x1000_u64;
        for field in [
            &mut descriptor.input_f16,
            &mut descriptor.trellis_u16,
            &mut descriptor.suh_f16,
            &mut descriptor.svh_f16,
            &mut descriptor.rotated_input_f16,
            &mut descriptor.projected_f16,
            &mut descriptor.output_f16,
            &mut descriptor.validation_error_u32,
        ] {
            *field = pointer;
            pointer += 0x100;
        }
        descriptor.workspace_bytes =
            exl3_workspace_bytes(1, descriptor.logical_k, descriptor.logical_n).unwrap();
        let valid = descriptor;

        descriptor.logical_n += 128;
        assert_eq!(
            validate_exl3_descriptor(&descriptor),
            Err(KernelError::Shape)
        );
        descriptor = valid;
        descriptor.trellis_u16 += 2;
        assert_eq!(
            validate_exl3_descriptor(&descriptor),
            Err(KernelError::Alignment)
        );
        descriptor = valid;
        descriptor.workspace_bytes -= 1;
        assert!(matches!(
            validate_exl3_descriptor(&descriptor),
            Err(KernelError::Workspace { .. })
        ));
    }

    #[test]
    fn every_required_fc2_decode_m_validates() {
        for rows in [1, 2, 4, 8, 16, 32, 64, 128] {
            let assignments = rows * TOP_K;
            let mut descriptor = Fc2Descriptor::new(LaunchGeometry {
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
                &mut descriptor.activation_values,
                &mut descriptor.activation_scales,
                &mut descriptor.activation_global_scales,
                &mut descriptor.assignment_down_f32,
                &mut descriptor.token_output_f32,
                &mut descriptor.slot_assignment_u32,
                &mut descriptor.validation_error_u32,
            ] {
                *field = pointer;
                pointer += 0x100;
            }
            descriptor.workspace_bytes = fc2_workspace_bytes(rows, assignments).unwrap();
            validate_fc2_descriptor(&descriptor).unwrap();
        }
    }

    #[test]
    fn fc2_workspace_includes_deterministic_scatter_state() {
        assert_eq!(fc2_workspace_bytes(1, 8).unwrap(), 356_420);
        assert_eq!(fc2_grouped_workspace_bytes(1, 8).unwrap(), 385_092);
        assert!(
            fc2_grouped_workspace_bytes(128, 1_024).unwrap()
                > fc2_workspace_bytes(128, 1_024).unwrap()
        );
        assert_eq!(fc2_workspace_bytes(0, 8), Err(KernelError::Shape));
        assert_eq!(fc2_workspace_bytes(1, 9), Err(KernelError::Shape));
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

    #[test]
    fn grouped_sfa_uses_expert_local_padded_slabs() {
        let mut offsets = [0_u32; EXPERTS as usize + 1];
        for offset in &mut offsets[1..=8] {
            *offset = 1;
        }
        for offset in &mut offsets[9..] {
            *offset = 8;
        }
        for expert in 1..8 {
            offsets[expert + 1] = expert as u32 + 1;
        }
        let plan = grouped_sfa_plan(&offsets).unwrap();
        assert_eq!(plan.active_experts, 8);
        assert_eq!(plan.total_bytes, 8 * 128 * SFA_BYTES_PER_PADDED_ROW);
        assert_eq!(plan.expert_byte_offsets[1], 128 * SFA_BYTES_PER_PADDED_ROW);
        assert_eq!(plan.expert_byte_offsets[8], plan.total_bytes);
        assert_eq!(plan.expert_byte_offsets[256], plan.total_bytes);
    }

    #[test]
    fn grouped_sfa_rejects_non_monotonic_or_oversized_offsets() {
        let mut non_monotonic = [0_u32; EXPERTS as usize + 1];
        non_monotonic[1] = 2;
        non_monotonic[2] = 1;
        assert_eq!(grouped_sfa_plan(&non_monotonic), Err(KernelError::Shape));

        let mut oversized = [0_u32; EXPERTS as usize + 1];
        oversized[EXPERTS as usize] = 65_536;
        assert_eq!(grouped_sfa_plan(&oversized), Err(KernelError::Shape));
    }

    #[test]
    fn grouped_sfa_capacity_covers_real_plans() {
        for assignments in [1_u32, 8, 128, 256, 257, 2_048, 65_535] {
            let active = assignments.min(EXPERTS);
            let mut offsets = [0_u32; EXPERTS as usize + 1];
            for expert in 0..active {
                offsets[expert as usize + 1] = expert + 1;
            }
            offsets[active as usize + 1..].fill(active);
            offsets[EXPERTS as usize] = assignments;
            let plan = grouped_sfa_plan(&offsets).unwrap();
            assert!(plan.total_bytes <= grouped_sfa_capacity_bytes(assignments).unwrap());
            assert!(
                grouped_workspace_bytes(assignments).unwrap()
                    >= workspace_bytes(assignments).unwrap()
            );
        }
        assert_eq!(
            grouped_sfa_capacity_bytes(256).unwrap(),
            256 * 128 * SFA_BYTES_PER_PADDED_ROW
        );
    }

    #[test]
    fn grouped_active_experts_require_exact_expert_major_ranges() {
        let routes = [0_u16, 0, 17, 255, 255];
        let mut offsets = [0_u32; EXPERTS as usize + 1];
        offsets[1..=17].fill(2);
        offsets[18..=255].fill(3);
        offsets[256] = 5;
        assert_eq!(
            active_experts_for_grouped(&routes, &offsets).unwrap(),
            [0, 17, 255]
        );

        let mut unsorted = routes;
        unsorted.swap(1, 2);
        assert_eq!(
            active_experts_for_grouped(&unsorted, &offsets),
            Err(KernelError::Shape)
        );
        offsets[256] = 4;
        assert_eq!(
            active_experts_for_grouped(&routes, &offsets),
            Err(KernelError::Shape)
        );
    }
}
