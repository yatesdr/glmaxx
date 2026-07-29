use std::{
    fmt::Write,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use glm_engine::StepMode;

use crate::ServingStepObservation;

const LATENCY_BOUNDS_US: [u64; 16] = [
    100, 250, 500, 1_000, 2_500, 5_000, 10_000, 25_000, 50_000, 100_000, 250_000, 500_000,
    1_000_000, 2_500_000, 5_000_000, 10_000_000,
];
const MAXIMUM_GRAPH_METRIC_ID: usize = 4_095;

pub(crate) struct ServingMetrics {
    submitted: AtomicU64,
    rejected: AtomicU64,
    completed: AtomicU64,
    cancelled: AtomicU64,
    failed: AtomicU64,
    slow_consumers: AtomicU64,
    prefix_cached_tokens: AtomicU64,
    prompt_computed_tokens: AtomicU64,
    draft_prompt_restored_tokens: AtomicU64,
    draft_prompt_computed_tokens: AtomicU64,
    output_tokens: AtomicU64,
    accepted_draft_tokens: AtomicU64,
    admitted_requests_by_mtp_depth: [AtomicU64; 7],
    verify_steps_by_mtp_depth: [AtomicU64; 7],
    accepted_draft_tokens_by_mtp_depth: [AtomicU64; 7],
    accepted_draft_tokens_by_ordinal: [AtomicU64; 6],
    terminated_stop: AtomicU64,
    terminated_length: AtomicU64,
    collective_tp_bytes: AtomicU64,
    collective_dcp_ckv_bytes: AtomicU64,
    collective_dcp_query_bytes: AtomicU64,
    collective_dcp_candidate_bytes: AtomicU64,
    collective_dcp_partial_bytes: AtomicU64,
    collective_sampling_bytes: AtomicU64,
    real_sequence_rows: AtomicU64,
    bucket_sequence_rows: AtomicU64,
    real_query_rows: AtomicU64,
    bucket_query_rows: AtomicU64,
    tokenization_time: LatencyHistogram,
    queue_time: LatencyHistogram,
    prefix_resolution_time: LatencyHistogram,
    admission_to_first_token: LatencyHistogram,
    ttft: LatencyHistogram,
    itl: LatencyHistogram,
    request_time: LatencyHistogram,
    step_worker_time: [LatencyHistogram; 3],
    step_host_time: [LatencyHistogram; 3],
    step_total_time: [LatencyHistogram; 3],
    graph_selections: [[AtomicU64; MAXIMUM_GRAPH_METRIC_ID + 1]; 3],
    graph_selection_overflow: AtomicU64,
}

impl ServingMetrics {
    pub fn new() -> Self {
        Self {
            submitted: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            completed: AtomicU64::new(0),
            cancelled: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            slow_consumers: AtomicU64::new(0),
            prefix_cached_tokens: AtomicU64::new(0),
            prompt_computed_tokens: AtomicU64::new(0),
            draft_prompt_restored_tokens: AtomicU64::new(0),
            draft_prompt_computed_tokens: AtomicU64::new(0),
            output_tokens: AtomicU64::new(0),
            accepted_draft_tokens: AtomicU64::new(0),
            admitted_requests_by_mtp_depth: std::array::from_fn(|_| AtomicU64::new(0)),
            verify_steps_by_mtp_depth: std::array::from_fn(|_| AtomicU64::new(0)),
            accepted_draft_tokens_by_mtp_depth: std::array::from_fn(|_| AtomicU64::new(0)),
            accepted_draft_tokens_by_ordinal: std::array::from_fn(|_| AtomicU64::new(0)),
            terminated_stop: AtomicU64::new(0),
            terminated_length: AtomicU64::new(0),
            collective_tp_bytes: AtomicU64::new(0),
            collective_dcp_ckv_bytes: AtomicU64::new(0),
            collective_dcp_query_bytes: AtomicU64::new(0),
            collective_dcp_candidate_bytes: AtomicU64::new(0),
            collective_dcp_partial_bytes: AtomicU64::new(0),
            collective_sampling_bytes: AtomicU64::new(0),
            real_sequence_rows: AtomicU64::new(0),
            bucket_sequence_rows: AtomicU64::new(0),
            real_query_rows: AtomicU64::new(0),
            bucket_query_rows: AtomicU64::new(0),
            tokenization_time: LatencyHistogram::new(),
            queue_time: LatencyHistogram::new(),
            prefix_resolution_time: LatencyHistogram::new(),
            admission_to_first_token: LatencyHistogram::new(),
            ttft: LatencyHistogram::new(),
            itl: LatencyHistogram::new(),
            request_time: LatencyHistogram::new(),
            step_worker_time: std::array::from_fn(|_| LatencyHistogram::new()),
            step_host_time: std::array::from_fn(|_| LatencyHistogram::new()),
            step_total_time: std::array::from_fn(|_| LatencyHistogram::new()),
            graph_selections: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU64::new(0))),
            graph_selection_overflow: AtomicU64::new(0),
        }
    }

    pub fn observe_queue(&self, duration: Duration) {
        self.queue_time.observe(duration);
    }

    pub fn increment_submitted(&self) {
        atomic_add(&self.submitted, 1);
    }

    pub fn increment_rejected(&self) {
        atomic_add(&self.rejected, 1);
    }

    pub fn increment_completed(&self) {
        atomic_add(&self.completed, 1);
    }

    pub fn increment_cancelled(&self) {
        atomic_add(&self.cancelled, 1);
    }

    pub fn increment_failed(&self) {
        atomic_add(&self.failed, 1);
    }

    pub fn increment_slow_consumers(&self) {
        atomic_add(&self.slow_consumers, 1);
    }

    pub fn observe_tokenization(&self, duration: Duration) {
        self.tokenization_time.observe(duration);
    }

    pub fn observe_prefix_resolution(&self, duration: Duration) {
        self.prefix_resolution_time.observe(duration);
    }

    pub fn observe_admission_to_first_token(&self, duration: Duration) {
        self.admission_to_first_token.observe(duration);
    }

    pub fn observe_ttft(&self, duration: Duration) {
        self.ttft.observe(duration);
    }

    pub fn observe_itl(&self, duration: Duration) {
        self.itl.observe(duration);
    }

    pub fn observe_request_time(&self, duration: Duration) {
        self.request_time.observe(duration);
    }

    pub fn add_prefix_restored(&self, tokens: u32, draft: bool) {
        atomic_add(&self.prefix_cached_tokens, u64::from(tokens));
        if draft {
            atomic_add(&self.draft_prompt_restored_tokens, u64::from(tokens));
        }
    }

    pub fn add_prompt_computed(&self, tokens: u32, draft: bool) {
        atomic_add(&self.prompt_computed_tokens, u64::from(tokens));
        if draft {
            atomic_add(&self.draft_prompt_computed_tokens, u64::from(tokens));
        }
    }

    pub fn observe_admitted_mtp_depth(&self, depth: u8) {
        if let Some(counter) = self.admitted_requests_by_mtp_depth.get(usize::from(depth)) {
            atomic_add(counter, 1);
        }
    }

    pub fn observe_output_token(
        &self,
        accepted_draft: bool,
        mtp_depth: u8,
        draft_ordinal: Option<u8>,
    ) {
        atomic_add(&self.output_tokens, 1);
        if accepted_draft {
            atomic_add(&self.accepted_draft_tokens, 1);
            if let Some(counter) = self
                .accepted_draft_tokens_by_mtp_depth
                .get(usize::from(mtp_depth))
            {
                atomic_add(counter, 1);
            }
            if let Some(counter) = draft_ordinal.and_then(|ordinal| {
                self.accepted_draft_tokens_by_ordinal
                    .get(usize::from(ordinal))
            }) {
                atomic_add(counter, 1);
            }
        }
    }

    pub fn observe_termination(&self, reason: &str) {
        match reason {
            "stop" => {
                atomic_add(&self.terminated_stop, 1);
            }
            "length" => {
                atomic_add(&self.terminated_length, 1);
            }
            _ => {}
        }
    }

    pub fn observe_step(&self, observation: &ServingStepObservation) {
        let Some(mode_index) = mode_index(observation.mode) else {
            return;
        };
        self.step_worker_time[mode_index].observe(observation.worker_round_trip);
        self.step_host_time[mode_index].observe(observation.coordinator_overhead);
        self.step_total_time[mode_index].observe(observation.total_step_time);
        if observation.mode == StepMode::Verify
            && let Some(counter) = self
                .verify_steps_by_mtp_depth
                .get(usize::from(observation.mtp_depth))
        {
            atomic_add(counter, 1);
        }
        atomic_add(
            &self.real_sequence_rows,
            u64::from(observation.real_sequences),
        );
        atomic_add(
            &self.bucket_sequence_rows,
            u64::from(observation.bucket_sequences),
        );
        atomic_add(
            &self.real_query_rows,
            u64::from(observation.real_query_rows),
        );
        atomic_add(
            &self.bucket_query_rows,
            u64::from(observation.bucket_query_rows),
        );
        atomic_add(
            &self.collective_tp_bytes,
            observation.collectives.tp_reduce_bytes,
        );
        atomic_add(
            &self.collective_dcp_ckv_bytes,
            observation.collectives.dcp_packed_ckv_bytes,
        );
        atomic_add(
            &self.collective_dcp_query_bytes,
            observation.collectives.dcp_query_gather_bytes,
        );
        atomic_add(
            &self.collective_dcp_candidate_bytes,
            observation.collectives.dcp_candidate_exchange_bytes,
        );
        atomic_add(
            &self.collective_dcp_partial_bytes,
            observation.collectives.dcp_partial_state_return_bytes,
        );
        atomic_add(
            &self.collective_sampling_bytes,
            observation.collectives.sampling_bytes,
        );
        match usize::try_from(observation.graph_id)
            .ok()
            .filter(|&graph_id| graph_id <= MAXIMUM_GRAPH_METRIC_ID)
        {
            Some(graph_id) => atomic_add(&self.graph_selections[mode_index][graph_id], 1),
            None => atomic_add(&self.graph_selection_overflow, 1),
        }
    }

    pub fn render(&self, active: usize, fatal: bool) -> String {
        let mut output = String::new();
        counter(
            &mut output,
            "glmaxx_backend_submitted_total",
            self.submitted.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_backend_rejected_total",
            self.rejected.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_backend_completed_total",
            self.completed.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_backend_cancelled_total",
            self.cancelled.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_backend_failed_total",
            self.failed.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_backend_slow_consumers_total",
            self.slow_consumers.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_prefix_cached_tokens_total",
            self.prefix_cached_tokens.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_prompt_computed_tokens_total",
            self.prompt_computed_tokens.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_draft_prompt_restored_tokens_total",
            self.draft_prompt_restored_tokens.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_draft_prompt_computed_tokens_total",
            self.draft_prompt_computed_tokens.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_output_tokens_total",
            self.output_tokens.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_accepted_draft_tokens_total",
            self.accepted_draft_tokens.load(Ordering::Relaxed),
        );
        for depth in 0_u8..=6 {
            counter_with_label(
                &mut output,
                "glmaxx_admitted_requests_total",
                "mtp_depth",
                depth,
                self.admitted_requests_by_mtp_depth[usize::from(depth)].load(Ordering::Relaxed),
            );
            counter_with_label(
                &mut output,
                "glmaxx_verify_steps_total",
                "mtp_depth",
                depth,
                self.verify_steps_by_mtp_depth[usize::from(depth)].load(Ordering::Relaxed),
            );
            counter_with_label(
                &mut output,
                "glmaxx_accepted_draft_tokens_by_depth_total",
                "mtp_depth",
                depth,
                self.accepted_draft_tokens_by_mtp_depth[usize::from(depth)].load(Ordering::Relaxed),
            );
        }
        for ordinal in 0_u8..6 {
            counter_with_label(
                &mut output,
                "glmaxx_accepted_draft_tokens_by_ordinal_total",
                "draft_ordinal",
                ordinal,
                self.accepted_draft_tokens_by_ordinal[usize::from(ordinal)].load(Ordering::Relaxed),
            );
        }
        counter(
            &mut output,
            "glmaxx_termination_stop_total",
            self.terminated_stop.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_termination_length_total",
            self.terminated_length.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_collective_tp_bytes_total",
            self.collective_tp_bytes.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_collective_dcp_ckv_bytes_total",
            self.collective_dcp_ckv_bytes.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_collective_dcp_query_bytes_total",
            self.collective_dcp_query_bytes.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_collective_dcp_candidate_bytes_total",
            self.collective_dcp_candidate_bytes.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_collective_dcp_partial_bytes_total",
            self.collective_dcp_partial_bytes.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_collective_sampling_bytes_total",
            self.collective_sampling_bytes.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_scheduler_real_sequence_rows_total",
            self.real_sequence_rows.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_scheduler_bucket_sequence_rows_total",
            self.bucket_sequence_rows.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_scheduler_real_query_rows_total",
            self.real_query_rows.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_scheduler_bucket_query_rows_total",
            self.bucket_query_rows.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_graph_selection_overflow_total",
            self.graph_selection_overflow.load(Ordering::Relaxed),
        );
        counter(
            &mut output,
            "glmaxx_backend_active_requests",
            u64::try_from(active).unwrap_or(u64::MAX),
        );
        counter(&mut output, "glmaxx_backend_fatal", u64::from(fatal));
        self.tokenization_time
            .render("glmaxx_tokenization_time_us", &mut output);
        self.queue_time.render("glmaxx_queue_time_us", &mut output);
        self.prefix_resolution_time
            .render("glmaxx_prefix_resolution_time_us", &mut output);
        self.admission_to_first_token
            .render("glmaxx_admission_to_first_token_us", &mut output);
        self.ttft.render("glmaxx_ttft_us", &mut output);
        self.itl.render("glmaxx_itl_us", &mut output);
        self.request_time
            .render("glmaxx_request_time_us", &mut output);
        for (index, mode) in ["prefill", "decode", "verify"].into_iter().enumerate() {
            self.step_worker_time[index].render(
                &format!("glmaxx_step_worker_round_trip_us_{mode}"),
                &mut output,
            );
            self.step_host_time[index].render(
                &format!("glmaxx_step_coordinator_overhead_us_{mode}"),
                &mut output,
            );
            self.step_total_time[index]
                .render(&format!("glmaxx_step_total_time_us_{mode}"), &mut output);
        }
        for (mode_index, mode) in ["prefill", "decode", "verify"].into_iter().enumerate() {
            for (graph_id, count) in self.graph_selections[mode_index].iter().enumerate() {
                let count = count.load(Ordering::Relaxed);
                if count != 0 {
                    let _ = writeln!(
                        output,
                        "glmaxx_graph_selections_total{{graph_id=\"{graph_id}\",mode=\"{mode}\"}} {count}"
                    );
                }
            }
        }
        output
    }
}

struct LatencyHistogram {
    buckets: [AtomicU64; LATENCY_BOUNDS_US.len() + 1],
    count: AtomicU64,
    sum_us: AtomicU64,
    max_us: AtomicU64,
}

impl LatencyHistogram {
    fn new() -> Self {
        Self {
            buckets: std::array::from_fn(|_| AtomicU64::new(0)),
            count: AtomicU64::new(0),
            sum_us: AtomicU64::new(0),
            max_us: AtomicU64::new(0),
        }
    }

    fn observe(&self, duration: Duration) {
        let microseconds = u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
        let bucket = LATENCY_BOUNDS_US
            .iter()
            .position(|&bound| microseconds <= bound)
            .unwrap_or(LATENCY_BOUNDS_US.len());
        atomic_add(&self.buckets[bucket], 1);
        atomic_add(&self.count, 1);
        atomic_add(&self.sum_us, microseconds);
        self.max_us.fetch_max(microseconds, Ordering::Relaxed);
    }

    fn render(&self, name: &str, output: &mut String) {
        let mut cumulative = 0_u64;
        for (index, &bound) in LATENCY_BOUNDS_US.iter().enumerate() {
            cumulative = cumulative.saturating_add(self.buckets[index].load(Ordering::Relaxed));
            let _ = writeln!(output, "{name}_bucket{{le=\"{bound}\"}} {cumulative}");
        }
        cumulative = cumulative
            .saturating_add(self.buckets[LATENCY_BOUNDS_US.len()].load(Ordering::Relaxed));
        let _ = writeln!(output, "{name}_bucket{{le=\"+Inf\"}} {cumulative}");
        let _ = writeln!(
            output,
            "{name}_count {}",
            self.count.load(Ordering::Relaxed)
        );
        let _ = writeln!(output, "{name}_sum {}", self.sum_us.load(Ordering::Relaxed));
        let _ = writeln!(output, "{name}_max {}", self.max_us.load(Ordering::Relaxed));
    }
}

fn counter(output: &mut String, name: &str, value: u64) {
    let _ = writeln!(output, "{name} {value}");
}

fn counter_with_label(output: &mut String, name: &str, label: &str, label_value: u8, value: u64) {
    let _ = writeln!(output, "{name}{{{label}=\"{label_value}\"}} {value}");
}

fn atomic_add(counter: &AtomicU64, value: u64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(value))
    });
}

const fn mode_index(mode: StepMode) -> Option<usize> {
    match mode {
        StepMode::Prefill => Some(0),
        StepMode::Decode => Some(1),
        StepMode::Verify => Some(2),
        StepMode::Mixed | StepMode::CacheOnly => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CollectivePayloadObservation;

    #[test]
    fn histograms_are_cumulative_and_step_bytes_remain_exclusive() {
        let metrics = ServingMetrics::new();
        metrics.observe_queue(Duration::from_micros(100));
        metrics.observe_queue(Duration::from_micros(101));
        metrics.output_tokens.store(u64::MAX, Ordering::Relaxed);
        metrics.observe_output_token(false, 0, None);
        metrics.observe_admitted_mtp_depth(3);
        metrics.observe_output_token(true, 3, Some(1));
        let observation = ServingStepObservation {
            step_id: 1,
            mode: StepMode::Decode,
            graph_id: 9,
            real_sequences: 3,
            bucket_sequences: 4,
            real_query_rows: 3,
            bucket_query_rows: 4,
            scheduled_prompt_tokens: 0,
            mtp_depth: 0,
            collective_count: 5,
            collective_schedule_hash: [1; 32],
            collectives: CollectivePayloadObservation {
                tp_reduce_bytes: 10,
                dcp_query_gather_bytes: 20,
                dcp_candidate_exchange_bytes: 30,
                dcp_partial_state_return_bytes: 40,
                sampling_bytes: 50,
                ..CollectivePayloadObservation::default()
            },
            worker_round_trip: Duration::from_micros(10),
            coordinator_overhead: Duration::from_micros(5),
            total_step_time: Duration::from_micros(15),
        };
        metrics.observe_step(&observation);
        metrics.observe_step(&ServingStepObservation {
            graph_id: 4_096,
            ..observation
        });
        let rendered = metrics.render(2, false);
        assert!(rendered.contains("glmaxx_queue_time_us_bucket{le=\"100\"} 1\n"));
        assert!(rendered.contains("glmaxx_queue_time_us_bucket{le=\"250\"} 2\n"));
        assert!(rendered.contains("glmaxx_queue_time_us_count 2\n"));
        assert!(rendered.contains("glmaxx_output_tokens_total 18446744073709551615\n"));
        assert!(rendered.contains("glmaxx_collective_tp_bytes_total 20\n"));
        assert!(rendered.contains("glmaxx_collective_dcp_query_bytes_total 40\n"));
        assert!(rendered.contains("glmaxx_collective_dcp_candidate_bytes_total 60\n"));
        assert!(rendered.contains("glmaxx_collective_dcp_partial_bytes_total 80\n"));
        assert!(rendered.contains("glmaxx_collective_sampling_bytes_total 100\n"));
        assert!(rendered.contains("glmaxx_graph_selection_overflow_total 1\n"));
        assert!(rendered.contains("glmaxx_admitted_requests_total{mtp_depth=\"3\"} 1\n"));
        assert!(
            rendered.contains("glmaxx_accepted_draft_tokens_by_depth_total{mtp_depth=\"3\"} 1\n")
        );
        assert!(
            rendered
                .contains("glmaxx_accepted_draft_tokens_by_ordinal_total{draft_ordinal=\"1\"} 1\n")
        );
        assert!(
            rendered.contains("glmaxx_graph_selections_total{graph_id=\"9\",mode=\"decode\"} 1\n")
        );
    }
}
