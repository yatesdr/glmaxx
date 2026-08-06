use std::fmt;

use sha2::{Digest, Sha256};

use crate::{
    BatchSmokeProgram, CollectiveSchedule, Digest32, ExecutorGraphKind, GraphMemoryPlan,
    GraphProfileV3, PhysicalPlanError, StepMode, StepPlan,
};

pub const STEP_PROGRAM_BINDING_RECORD_BYTES: usize = 464;

const MAGIC: &[u8; 8] = b"G5SPBV1\0";
const BINDING_DOMAIN: &[u8] = b"glmaxx.step-program-binding.v1\0";
const PROGRAM_SET_DOMAIN: &[u8] = b"glmaxx.executor-graph-program-set.v1\0";
const IDENTITY_COUNT: usize = 12;
const HASH_INPUT_BYTES: usize = STEP_PROGRAM_BINDING_RECORD_BYTES - 32;

const TARGET_PROGRAM: usize = 2;
const MTP_PROGRAM: usize = 3;
const PROGRAM_SET: usize = 4;
const MODULE_SET: usize = 5;
const GRAPH_MEMORY_ABI: usize = 6;
const GRAPH_PROFILE_V3: usize = 7;
const GRAPH_MEMORY_PLAN: usize = 8;
const STEP_PLAN: usize = 10;
const COLLECTIVE_SCHEDULE: usize = 11;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepProgramBinding {
    graph_kind: ExecutorGraphKind,
    mtp_depth: u8,
    graph_id: u32,
    resident_generation: u64,
    module_generation: u64,
    identities: [Digest32; IDENTITY_COUNT],
    digest: Digest32,
}

impl StepProgramBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn for_batch_smoke(
        batch_program: BatchSmokeProgram,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
        physical_plan: GraphMemoryPlan,
        graph_profile: &GraphProfileV3,
        graph_memory_abi_sha256: Digest32,
        resident_generation: u64,
        module_generation: u64,
    ) -> Result<Self, StepProgramBindingError> {
        batch_program.validate()?;
        if plan.mtp_depth != 0
            || plan.mode == StepMode::Verify
            || plan.active_sequences != 4
            || plan.sequence_bucket != 4
            || match plan.mode {
                StepMode::Decode => plan.query_rows != 4 || plan.verifier_row_bucket != 4,
                StepMode::Prefill => {
                    plan.verifier_row_bucket != 0
                        || plan.query_rows > batch_program.prefill_row_bucket()
                }
                StepMode::Verify | StepMode::Mixed | StepMode::CacheOnly => true,
            }
        {
            return Err(StepProgramBindingError::Step);
        }
        let target_program_sha256 = batch_program.target_program_sha256();
        let graph_kind = graph_kind(plan.mode)?;
        let program_set_sha256 =
            executor_program_set_sha256(graph_kind, target_program_sha256, None)?;
        if batch_program.graph_profile_v3_sha256() != graph_profile.digest() {
            return Err(StepProgramBindingError::Program);
        }
        let mut binding = Self {
            graph_kind,
            mtp_depth: 0,
            graph_id: plan.graph_id,
            resident_generation,
            module_generation,
            identities: [
                batch_program.digest(),
                batch_program.resident_weight_generation_sha256(),
                target_program_sha256,
                [0; 32],
                program_set_sha256,
                batch_program.module_set_capability_sha256(),
                graph_memory_abi_sha256,
                graph_profile.digest(),
                physical_plan.digest(),
                batch_program.system_memory_plan_sha256(),
                plan.plan_hash,
                schedule.hash(),
            ],
            digest: [0; 32],
        };
        binding.validate(plan, schedule, physical_plan, graph_profile)?;
        binding.digest = binding.compute_digest();
        Ok(binding)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_batch_smoke_bytes(
        bytes: &[u8],
        batch_program: BatchSmokeProgram,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
        physical_plan: GraphMemoryPlan,
        graph_profile: &GraphProfileV3,
    ) -> Result<Self, StepProgramBindingError> {
        let decoded = Self::decode_record(bytes)?;
        let expected = Self::for_batch_smoke(
            batch_program,
            plan,
            schedule,
            physical_plan,
            graph_profile,
            decoded.identities[GRAPH_MEMORY_ABI],
            decoded.resident_generation,
            decoded.module_generation,
        )?;
        if decoded != expected {
            return Err(StepProgramBindingError::Program);
        }
        Ok(decoded)
    }

    fn decode_record(bytes: &[u8]) -> Result<Self, StepProgramBindingError> {
        if bytes.len() != STEP_PROGRAM_BINDING_RECORD_BYTES
            || &bytes[..8] != MAGIC
            || read_u16(bytes, 8) != 1
            || usize::from(read_u16(bytes, 10)) != STEP_PROGRAM_BINDING_RECORD_BYTES
            || read_u32(bytes, 20) != 0
            || read_u64(bytes, 40) != 0
        {
            return Err(StepProgramBindingError::Header);
        }
        let flags = read_u16(bytes, 14);
        if flags & !1 != 0 {
            return Err(StepProgramBindingError::Reserved);
        }
        let binding = Self {
            graph_kind: decode_graph_kind(bytes[12])?,
            mtp_depth: bytes[13],
            graph_id: read_u32(bytes, 16),
            resident_generation: read_u64(bytes, 24),
            module_generation: read_u64(bytes, 32),
            identities: std::array::from_fn(|index| {
                let start = 48 + index * 32;
                bytes[start..start + 32].try_into().expect("fixed record")
            }),
            digest: bytes[HASH_INPUT_BYTES..].try_into().expect("fixed record"),
        };
        if (flags & 1 != 0) != binding.mtp_program_present() {
            return Err(StepProgramBindingError::MtpBinding);
        }
        binding.validate_self()?;
        if binding.compute_digest() != binding.digest {
            return Err(StepProgramBindingError::Hash);
        }
        Ok(binding)
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; STEP_PROGRAM_BINDING_RECORD_BYTES] {
        let mut bytes = [0_u8; STEP_PROGRAM_BINDING_RECORD_BYTES];
        bytes[..HASH_INPUT_BYTES].copy_from_slice(&self.hash_input());
        bytes[HASH_INPUT_BYTES..].copy_from_slice(&self.digest);
        bytes
    }

    #[must_use]
    pub const fn digest(self) -> Digest32 {
        self.digest
    }

    #[must_use]
    pub const fn resident_generation(self) -> u64 {
        self.resident_generation
    }

    #[must_use]
    pub const fn module_generation(self) -> u64 {
        self.module_generation
    }

    #[must_use]
    pub const fn target_program_sha256(self) -> Digest32 {
        self.identities[TARGET_PROGRAM]
    }

    #[must_use]
    pub const fn graph_memory_plan_sha256(self) -> Digest32 {
        self.identities[GRAPH_MEMORY_PLAN]
    }

    pub fn validate(
        self,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
        physical_plan: GraphMemoryPlan,
        graph_profile: &GraphProfileV3,
    ) -> Result<(), StepProgramBindingError> {
        self.validate_self()?;
        plan.verify(schedule)?;
        if self.graph_kind != graph_kind(plan.mode)?
            || self.mtp_depth != plan.mtp_depth
            || self.graph_id != plan.graph_id
            || self.identities[STEP_PLAN] != plan.plan_hash
            || self.identities[COLLECTIVE_SCHEDULE] != schedule.hash()
        {
            return Err(StepProgramBindingError::Step);
        }
        physical_plan.admit_step(plan)?;
        let request = physical_plan.request();
        if self.identities[GRAPH_MEMORY_PLAN] != physical_plan.digest()
            || request.identities[0] != self.identities[TARGET_PROGRAM]
            || request.identities[1] != self.identities[MTP_PROGRAM]
            || request.identities[2] != self.identities[PROGRAM_SET]
            || request.identities[4] != self.identities[MODULE_SET]
            || request.identities[5] != self.identities[COLLECTIVE_SCHEDULE]
        {
            return Err(StepProgramBindingError::PhysicalPlan);
        }
        graph_profile.admit(self.graph_id, physical_plan.digest())?;
        if graph_profile.digest() != self.identities[GRAPH_PROFILE_V3]
            || graph_profile.parent_profile_sha256() != request.identities[3]
        {
            return Err(StepProgramBindingError::GraphProfile);
        }
        if self.compute_digest() != self.digest && self.digest != [0; 32] {
            return Err(StepProgramBindingError::Hash);
        }
        Ok(())
    }

    pub fn verify_step(
        self,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
    ) -> Result<(), StepProgramBindingError> {
        self.validate_self()?;
        plan.verify(schedule)?;
        if self.graph_kind != graph_kind(plan.mode)?
            || self.mtp_depth != plan.mtp_depth
            || self.graph_id != plan.graph_id
            || self.identities[STEP_PLAN] != plan.plan_hash
            || self.identities[COLLECTIVE_SCHEDULE] != schedule.hash()
        {
            return Err(StepProgramBindingError::Step);
        }
        if self.compute_digest() != self.digest {
            return Err(StepProgramBindingError::Hash);
        }
        Ok(())
    }

    fn mtp_program_present(self) -> bool {
        self.identities[MTP_PROGRAM] != [0; 32]
    }

    fn validate_self(self) -> Result<(), StepProgramBindingError> {
        if self.graph_id == 0 || self.resident_generation == 0 || self.module_generation == 0 {
            return Err(StepProgramBindingError::Generation);
        }
        for (index, identity) in self.identities.iter().enumerate() {
            if index != MTP_PROGRAM && *identity == [0; 32] {
                return Err(StepProgramBindingError::Identity);
            }
        }
        let mtp_present = self.mtp_program_present();
        if (self.graph_kind == ExecutorGraphKind::Verify) != mtp_present
            || mtp_present != (self.mtp_depth != 0)
        {
            return Err(StepProgramBindingError::MtpBinding);
        }
        let expected = executor_program_set_sha256(
            self.graph_kind,
            self.identities[TARGET_PROGRAM],
            mtp_present.then_some(self.identities[MTP_PROGRAM]),
        )?;
        if expected != self.identities[PROGRAM_SET] {
            return Err(StepProgramBindingError::Program);
        }
        Ok(())
    }

    fn compute_digest(self) -> Digest32 {
        let mut hasher = Sha256::new();
        hasher.update(BINDING_DOMAIN);
        hasher.update(self.hash_input());
        hasher.finalize().into()
    }

    fn hash_input(self) -> [u8; HASH_INPUT_BYTES] {
        let mut bytes = [0_u8; HASH_INPUT_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(STEP_PROGRAM_BINDING_RECORD_BYTES as u16).to_le_bytes());
        bytes[12] = self.graph_kind as u8;
        bytes[13] = self.mtp_depth;
        bytes[14..16].copy_from_slice(&u16::from(self.mtp_program_present()).to_le_bytes());
        bytes[16..20].copy_from_slice(&self.graph_id.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.resident_generation.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.module_generation.to_le_bytes());
        for (index, identity) in self.identities.iter().enumerate() {
            let start = 48 + index * 32;
            bytes[start..start + 32].copy_from_slice(identity);
        }
        bytes
    }
}

pub fn executor_program_set_sha256(
    graph_kind: ExecutorGraphKind,
    target_program_sha256: Digest32,
    mtp_program_sha256: Option<Digest32>,
) -> Result<Digest32, StepProgramBindingError> {
    if target_program_sha256 == [0; 32]
        || mtp_program_sha256.is_some_and(|digest| digest == [0; 32])
        || (graph_kind == ExecutorGraphKind::Verify) != mtp_program_sha256.is_some()
    {
        return Err(StepProgramBindingError::MtpBinding);
    }
    let mut hasher = Sha256::new();
    hasher.update(PROGRAM_SET_DOMAIN);
    hasher.update((graph_kind as u32).to_le_bytes());
    hasher.update(target_program_sha256);
    hasher.update([u8::from(mtp_program_sha256.is_some())]);
    hasher.update([0; 7]);
    hasher.update(mtp_program_sha256.unwrap_or([0; 32]));
    Ok(hasher.finalize().into())
}

fn graph_kind(mode: StepMode) -> Result<ExecutorGraphKind, StepProgramBindingError> {
    match mode {
        StepMode::Prefill => Ok(ExecutorGraphKind::Prefill),
        StepMode::Decode => Ok(ExecutorGraphKind::Decode),
        StepMode::Verify => Ok(ExecutorGraphKind::Verify),
        StepMode::Mixed | StepMode::CacheOnly => Err(StepProgramBindingError::Step),
    }
}

fn decode_graph_kind(value: u8) -> Result<ExecutorGraphKind, StepProgramBindingError> {
    match value {
        1 => Ok(ExecutorGraphKind::Prefill),
        2 => Ok(ExecutorGraphKind::Decode),
        3 => Ok(ExecutorGraphKind::Verify),
        _ => Err(StepProgramBindingError::Header),
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixed record"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed record"))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed record"))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepProgramBindingError {
    Header,
    Reserved,
    Identity,
    Generation,
    MtpBinding,
    Program,
    Step,
    PhysicalPlan,
    GraphProfile,
    Hash,
    Plan(crate::PlanError),
    ProgramRecord(crate::ProgramRecordError),
    Physical(PhysicalPlanError),
}

impl fmt::Display for StepProgramBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for StepProgramBindingError {}

impl From<crate::PlanError> for StepProgramBindingError {
    fn from(value: crate::PlanError) -> Self {
        Self::Plan(value)
    }
}

impl From<crate::ProgramRecordError> for StepProgramBindingError {
    fn from(value: crate::ProgramRecordError) -> Self {
        Self::ProgramRecord(value)
    }
}

impl From<PhysicalPlanError> for StepProgramBindingError {
    fn from(value: PhysicalPlanError) -> Self {
        Self::Physical(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AttentionTransport, CollectiveKind, CollectiveOp, GraphArenaTable, GraphBufferUse,
        GraphBufferUseTable, GraphClassSpan, GraphClassSpanTable, GraphMemoryPlanRequest,
        ProductionProfile, StepPlanRequest, TP_RANK_MASK,
    };

    fn digest_value(value: u8) -> Digest32 {
        [value; 32]
    }

    fn schedule() -> CollectiveSchedule {
        CollectiveSchedule::new(vec![CollectiveOp {
            ordinal: 0,
            kind: CollectiveKind::LogitsArgmax,
            route_id: 1,
            payload_bytes: 16,
            participant_mask: TP_RANK_MASK,
        }])
        .unwrap()
    }

    fn plan(schedule: &CollectiveSchedule, step_id: u64) -> StepPlan {
        StepPlan::build(
            StepPlanRequest {
                epoch: 1,
                step_id,
                mode: StepMode::Decode,
                active_sequences: 4,
                sequence_bucket: 4,
                scheduled_prompt_tokens: 0,
                query_rows: 4,
                verifier_row_bucket: 4,
                mtp_depth: 0,
                graph_id: 7,
                tp_route_id: 1,
                dcp_route_id: 1,
                attention_transport: AttentionTransport::DecodeQueryLse,
                sampling_route_id: 1,
                sequence_table_generation: 5,
            },
            schedule,
        )
        .unwrap()
    }

    fn physical_plan(
        schedule: &CollectiveSchedule,
        target_program: Digest32,
        parent_profile: Digest32,
        module_set: Digest32,
    ) -> GraphMemoryPlan {
        let arenas = GraphArenaTable::new(
            [256, 4096, 4096, 4096, 4096, 4096, 256, 4096, 4096, 4096],
            [256; 10],
        )
        .unwrap();
        let spans = GraphClassSpanTable::new(
            std::array::from_fn(|index| {
                let class_id = u16::try_from(index + 1).unwrap();
                if class_id == 1 {
                    GraphClassSpan::present(class_id, 0, 256, 256, 1, 1, 1, false).unwrap()
                } else {
                    GraphClassSpan::absent(class_id).unwrap()
                }
            }),
            &arenas,
        )
        .unwrap();
        let uses = GraphBufferUseTable::new(
            vec![
                GraphBufferUse::new(
                    1,
                    0,
                    2,
                    1,
                    true,
                    true,
                    1,
                    false,
                    0,
                    256,
                    256,
                    digest_value(40),
                )
                .unwrap(),
                GraphBufferUse::new(
                    2,
                    0,
                    8,
                    0,
                    true,
                    false,
                    1,
                    false,
                    0,
                    256,
                    256,
                    digest_value(41),
                )
                .unwrap(),
            ],
            &spans,
            &arenas,
        )
        .unwrap();
        let program_set =
            executor_program_set_sha256(ExecutorGraphKind::Decode, target_program, None).unwrap();
        GraphMemoryPlan::new(
            GraphMemoryPlanRequest {
                graph_id: 7,
                graph_kind: ExecutorGraphKind::Decode,
                attention_transport: AttentionTransport::DecodeQueryLse,
                mtp_depth: 0,
                sequence_bucket: 4,
                row_bucket: 4,
                token_bucket: 0,
                identities: [
                    target_program,
                    [0; 32],
                    program_set,
                    parent_profile,
                    module_set,
                    schedule.hash(),
                    digest_value(26),
                    digest_value(27),
                ],
            },
            &arenas,
            &spans,
            &uses,
        )
        .unwrap()
    }

    fn fixture() -> (
        BatchSmokeProgram,
        StepPlan,
        CollectiveSchedule,
        GraphMemoryPlan,
        GraphProfileV3,
        StepProgramBinding,
    ) {
        let schedule = schedule();
        let plan = plan(&schedule, 1);
        let target = digest_value(20);
        let parent = digest_value(21);
        let module = digest_value(22);
        let physical = physical_plan(&schedule, target, parent, module);
        let profile = GraphProfileV3::new(parent, vec![(7, physical.digest())]).unwrap();
        let mut identities: [Digest32; 20] =
            std::array::from_fn(|index| digest_value(u8::try_from(index + 1).unwrap()));
        identities[3] = digest_value(23);
        identities[4] = target;
        identities[5] = profile.digest();
        identities[8] = module;
        identities[10] = digest_value(24);
        let batch =
            BatchSmokeProgram::new(ProductionProfile::CapacityExl3, 256, identities).unwrap();
        let binding = StepProgramBinding::for_batch_smoke(
            batch,
            &plan,
            &schedule,
            physical,
            &profile,
            digest_value(25),
            11,
            12,
        )
        .unwrap();
        (batch, plan, schedule, physical, profile, binding)
    }

    #[test]
    fn exact_binding_round_trips_and_revalidates_every_typed_parent() {
        let (_batch, plan, schedule, physical, profile, binding) = fixture();
        assert_eq!(
            binding.validate(&plan, &schedule, physical, &profile),
            Ok(())
        );
        assert_eq!(binding.verify_step(&plan, &schedule), Ok(()));
        assert_eq!(
            StepProgramBinding::from_batch_smoke_bytes(
                &binding.to_bytes(),
                _batch,
                &plan,
                &schedule,
                physical,
                &profile,
            ),
            Ok(binding)
        );
        assert_ne!(binding.digest(), [0; 32]);
    }

    #[test]
    fn step_plan_physical_plan_and_record_drift_fail_closed() {
        let (_batch, original_plan, schedule, physical, profile, binding) = fixture();
        let changed_step = plan(&schedule, 2);
        assert_eq!(
            binding.verify_step(&changed_step, &schedule),
            Err(StepProgramBindingError::Step)
        );

        let changed_physical = physical_plan(
            &schedule,
            binding.target_program_sha256(),
            profile.parent_profile_sha256(),
            digest_value(31),
        );
        assert!(matches!(
            binding.validate(&original_plan, &schedule, changed_physical, &profile),
            Err(StepProgramBindingError::PhysicalPlan)
        ));
        assert_ne!(physical.digest(), changed_physical.digest());

        let mut bytes = binding.to_bytes();
        bytes[48] ^= 1;
        assert_eq!(
            StepProgramBinding::from_batch_smoke_bytes(
                &bytes,
                _batch,
                &original_plan,
                &schedule,
                physical,
                &profile,
            ),
            Err(StepProgramBindingError::Hash)
        );
    }
}
