---
name: workflow
description: Agent iteration loops and tool execution
---

# Workflow

1. **Think**: Analyze context, select tools, update `ConductorPlan`.
2. **Act**: Execute `StepType::Execute` operations concurrently if no sequential dependencies.
3. **Observe**: Run `StepType::Verify` tools to validate state mutability.
4. If success, commit step to `EpisodicMemory`.
