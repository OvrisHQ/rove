//! Conductor Evaluator
//!
//! Evaluates the results of `PlanStep`s to determine success, detect hallucination
//! or infinite loops, and adjust the `ConductorPlan` dynamically.

use crate::conductor::types::{ConductorPlan, PlanStep, StepResult};
use anyhow::Result;
use std::collections::VecDeque;

/// Maximum number of recent log hashes to keep for loop detection
const LOOP_HISTORY_SIZE: usize = 5;

/// Heuristic Evaluator for the Conductor
#[derive(Default)]
pub struct Evaluator {
    /// Rolling window of recent step log hashes for loop detection
    recent_log_hashes: VecDeque<u64>,
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            recent_log_hashes: VecDeque::with_capacity(LOOP_HISTORY_SIZE),
        }
    }

    /// Evaluate a step result against the current plan.
    /// Returns `true` if the step succeeded and the plan can continue,
    /// or `false` if the plan needs to be revised or aborted.
    pub fn evaluate_step(
        &mut self,
        _plan: &ConductorPlan,
        _step: &PlanStep,
        result: &StepResult,
    ) -> Result<bool> {
        // 1. Explicit failure
        if !result.success {
            return Ok(false);
        }

        // 2. Detect error keywords in logs (soft failure)
        if result.logs.contains("error:") || result.logs.contains("Error:") {
            return Ok(false);
        }

        // 3. Loop detection: hash the logs and compare with recent history
        if self.detect_loop(&result.logs) {
            tracing::warn!(
                "Loop detected: step {} produced identical output to a recent step",
                result.step_id
            );
            return Ok(false);
        }

        // 4. Hallucination detection: empty output from an Execute step
        if result.context_extracted.is_empty() && result.logs.is_empty() {
            tracing::warn!(
                "Possible hallucination: step {} produced no output",
                result.step_id
            );
            return Ok(false);
        }

        Ok(true)
    }

    /// Detect if the current step output is a repeat of a recent step
    fn detect_loop(&mut self, logs: &str) -> bool {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        logs.hash(&mut hasher);
        let hash = hasher.finish();

        let is_repeat = self.recent_log_hashes.contains(&hash);

        // Maintain rolling window
        if self.recent_log_hashes.len() >= LOOP_HISTORY_SIZE {
            self.recent_log_hashes.pop_front();
        }
        self.recent_log_hashes.push_back(hash);

        is_repeat
    }

    /// Assess if the overall goal has been met
    pub fn is_goal_met(&self, plan: &ConductorPlan, completed_steps: &[StepResult]) -> bool {
        plan.steps.iter().all(|step| {
            completed_steps
                .iter()
                .any(|r| r.step_id == step.id && r.success)
        })
    }

    /// Reset the evaluator state for a new plan
    pub fn reset(&mut self) {
        self.recent_log_hashes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conductor::types::StepType;

    fn make_plan() -> ConductorPlan {
        ConductorPlan {
            id: "plan_1".to_string(),
            original_goal: "test".to_string(),
            steps: vec![
                PlanStep {
                    id: "s1".to_string(),
                    description: "step 1".to_string(),
                    step_type: StepType::Research,
                    dependencies: vec![],
                    expected_outcome: "done".to_string(),
                },
                PlanStep {
                    id: "s2".to_string(),
                    description: "step 2".to_string(),
                    step_type: StepType::Execute,
                    dependencies: vec!["s1".to_string()],
                    expected_outcome: "done".to_string(),
                },
            ],
            created_at: 0,
        }
    }

    fn make_step(id: &str) -> PlanStep {
        PlanStep {
            id: id.to_string(),
            description: "test".to_string(),
            step_type: StepType::Execute,
            dependencies: vec![],
            expected_outcome: "done".to_string(),
        }
    }

    fn make_result(step_id: &str, success: bool, logs: &str) -> StepResult {
        StepResult {
            step_id: step_id.to_string(),
            success,
            tools_used: vec![],
            logs: logs.to_string(),
            context_extracted: "some context".to_string(),
        }
    }

    #[test]
    fn test_evaluate_success() {
        let mut eval = Evaluator::new();
        let plan = make_plan();
        let step = make_step("s1");
        let result = make_result("s1", true, "all good");
        assert!(eval.evaluate_step(&plan, &step, &result).unwrap());
    }

    #[test]
    fn test_evaluate_explicit_failure() {
        let mut eval = Evaluator::new();
        let plan = make_plan();
        let step = make_step("s1");
        let result = make_result("s1", false, "something broke");
        assert!(!eval.evaluate_step(&plan, &step, &result).unwrap());
    }

    #[test]
    fn test_evaluate_error_in_logs() {
        let mut eval = Evaluator::new();
        let plan = make_plan();
        let step = make_step("s1");
        let result = make_result("s1", true, "compilation error: undefined variable");
        assert!(!eval.evaluate_step(&plan, &step, &result).unwrap());
    }

    #[test]
    fn test_loop_detection() {
        let mut eval = Evaluator::new();
        let plan = make_plan();
        let step = make_step("s1");

        let result1 = make_result("s1", true, "output A");
        assert!(eval.evaluate_step(&plan, &step, &result1).unwrap());

        let result2 = make_result("s1", true, "output B");
        assert!(eval.evaluate_step(&plan, &step, &result2).unwrap());

        // Repeat of result1's logs
        let result3 = make_result("s1", true, "output A");
        assert!(!eval.evaluate_step(&plan, &step, &result3).unwrap());
    }

    #[test]
    fn test_hallucination_detection() {
        let mut eval = Evaluator::new();
        let plan = make_plan();
        let step = make_step("s1");
        let result = StepResult {
            step_id: "s1".to_string(),
            success: true,
            tools_used: vec![],
            logs: String::new(),
            context_extracted: String::new(),
        };
        assert!(!eval.evaluate_step(&plan, &step, &result).unwrap());
    }

    #[test]
    fn test_is_goal_met() {
        let eval = Evaluator::new();
        let plan = make_plan();

        // Only s1 completed
        let partial = vec![make_result("s1", true, "done")];
        assert!(!eval.is_goal_met(&plan, &partial));

        // Both completed
        let full = vec![
            make_result("s1", true, "done"),
            make_result("s2", true, "done"),
        ];
        assert!(eval.is_goal_met(&plan, &full));

        // s2 failed
        let with_failure = vec![
            make_result("s1", true, "done"),
            make_result("s2", false, "failed"),
        ];
        assert!(!eval.is_goal_met(&plan, &with_failure));
    }

    #[test]
    fn test_reset() {
        let mut eval = Evaluator::new();
        let plan = make_plan();
        let step = make_step("s1");

        let result = make_result("s1", true, "output X");
        eval.evaluate_step(&plan, &step, &result).unwrap();

        eval.reset();

        // Same output should NOT trigger loop after reset
        let result2 = make_result("s1", true, "output X");
        assert!(eval.evaluate_step(&plan, &step, &result2).unwrap());
    }
}
