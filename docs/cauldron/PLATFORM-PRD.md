# Cauldron — Platform & Code Separation
## Product Requirements Document and Architecture Blueprint

**Status:** Proposed target architecture  
**Scope:** Separate the shared Cauldron Platform from the Cauldron Code application, define their responsibilities, and establish the autonomous development lifecycle.

---

# 1. Executive vision

Cauldron is an extensible application platform for running intelligent, agentic applications.

It should not be architected as one large application in which every capability is coupled to Code. Instead, Cauldron should provide a shared runtime and a collection of reusable platform capabilities. Individual applications consume those capabilities through explicit interfaces and permissions.

Cauldron Code is the first major application built on this platform. It is also the first proving ground for the Decision Engine, autonomous execution, knowledge acquisition, and the gated development lifecycle.

The distinction is fundamental:

- **Cauldron Platform** provides the infrastructure, runtime, shared services, application lifecycle, and common capabilities.
- **Cauldron Code** provides the specialized development environment, coding agents, execution workflows, and development-specific policies.
- **Decision Engine** is an independent reusable software dependency integrated into Code. It is not a platform-wide microservice by default.

The objective is to make Code work exceptionally well without making the platform dependent on Code.

# 2. Product architecture

## 2.1 High-level architecture

```text
CAULDRON PLATFORM
│
├── Application Runtime
│   ├── App installation and lifecycle
│   ├── App manifests and permissions
│   ├── App isolation
│   └── Runtime compatibility
│
├── Shared Model Runtime
│   ├── Local inference
│   ├── Remote model providers
│   ├── Model registry
│   ├── Model lifecycle manager
│   └── Resource scheduling
│
├── Shared Library
│   ├── Knowledge
│   ├── Skills
│   ├── Documents and assets
│   ├── Memories
│   └── Semantic definitions
│
├── Shared Event and Notification Services
│   ├── Event bus
│   ├── Notification inbox
│   ├── Desktop notifications
│   └── Event subscriptions
│
├── Shared Storage and Integrations
│
└── Application Store
    │
    └── CAULDRON CODE
        ├── IDE and development workspace
        ├── Agent orchestration
        ├── Terminals and worktrees
        ├── Git and CI integrations
        ├── Plan / Execute / Verify / Ship
        ├── Learn and knowledge acquisition
        ├── Librarian
        ├── Judge
        ├── Code-specific policies
        │
        └── Decision Engine (bundled dependency)
            ├── Decision lifecycle
            ├── Context and evidence
            ├── Policy evaluation
            ├── Model orchestration
            ├── Decision ledger contracts
            ├── Outcome tracking
            └── Typed decision events
```

The platform exposes capabilities. Applications decide how to use them.

## 2.2 Dependency rules

1. The platform must not import or depend on Cauldron Code.
2. Code must consume platform capabilities through stable interfaces.
3. The Decision Engine must remain independently buildable and reusable.
4. The Decision Engine must not require Cauldron, SQLite, a specific model provider, or a specific event bus.
5. The platform owns shared infrastructure; the application owns its specialized workflows.
6. Applications must not independently start competing copies of the platform model runtime.
7. Platform APIs and contracts must be versioned.

# 3. Cauldron Platform — requirements

The platform is the foundation on which every installed application runs.

It should offer a consistent environment for applications to access models, storage, knowledge, notifications, integrations, and shared resources.

## 3.1 Application runtime and lifecycle

The platform must support:

- Application installation, removal, updating, and rollback.
- Application manifests describing identity, version, dependencies, and required capabilities.
- Per-application storage namespaces.
- Application isolation and resource boundaries.
- Permission management and capability-based access.
- Platform compatibility checks.
- Application startup, shutdown, health checks, and lifecycle events.
- App Store discovery and distribution.

An application should declare its requirements instead of assuming unrestricted access to the host.

Example manifest capabilities:

```yaml
app:
  id: cauldron.code
  name: Cauldron Code

permissions:
  - models.invoke
  - library.read
  - library.write
  - storage.read
  - storage.write
  - notifications.publish
  - notifications.subscribe
  - decisions.create
  - tools.execute
  - git.access
```

The exact manifest format is an implementation decision. The important requirement is that capabilities are explicit and enforceable.

## 3.2 Shared model runtime

The platform must provide a common model interface accessible to every application.

Code should not own the model infrastructure. Code should request inference from the platform.

The model runtime should support:

**Local models**
- ONNX models through ONNX Runtime.
- Quantized LLM models through compatible runtimes such as llama.cpp.
- CPU and GPU inference where supported.
- Local embeddings and other supported model tasks.

**Remote models**
- Configurable provider adapters.
- API credentials managed by the platform.
- Provider fallback where permitted.
- Cost and usage tracking.

**Model management**
- Model registry and version metadata.
- Download, installation, loading, unloading, and updates.
- Hardware capability detection.
- RAM and VRAM monitoring.
- Concurrency and resource scheduling.
- Model prioritization between applications.
- CPU fallback and remote fallback.
- Privacy policies, including local-only execution.

Applications should be able to specify requirements such as:

```text
Task: code generation
Preferred model: local model A
Fallback: remote provider B
Privacy: remote inference allowed
Priority: normal
Maximum resource allocation: configured budget
```

The runtime chooses an available compatible execution path and reports the actual model, provider, version, and resource usage.

**Important:** ONNX and GGUF are not interchangeable formats. The platform must use compatible inference backends rather than presenting every model format as universally supported.

## 3.3 Shared Library

The Library is the platform's shared knowledge and resource layer.

It should make reusable information available to authorized applications without requiring each application to build a separate knowledge repository.

Potential resource categories:

- Skills and reusable procedures.
- Knowledge documents.
- Project-specific decisions.
- Global rules and conventions.
- Memories and contextual information.
- Semantic definitions.
- Files, assets, and references.
- Provenance and version history.

Applications should be able to:

- Read permitted Library resources.
- Create and update resources within authorized scopes.
- Search semantically.
- Subscribe to relevant changes.
- Propose information for promotion into shared knowledge.
- Preserve source, ownership, scope, and provenance.

The Library is not the same thing as the Decision Ledger.

The Library stores reusable knowledge. The Decision Ledger records decision history and evidence.

## 3.4 Event bus and notifications

Notifications must be a platform capability, not an implementation hidden inside Code or the Decision Engine.

The platform should expose an event system that applications and platform services can publish to and subscribe to.

The Notification Service consumes relevant events and handles delivery.

Initial notification surfaces:

- In-app inbox.
- Desktop notifications.
- Application activity feeds.

Future surfaces can include mobile push, email, and webhooks.

Required capabilities:

- Priority and urgency.
- Per-application preferences.
- Read and unread states.
- Deduplication and grouping.
- Quiet hours.
- Persistent notifications when the user is offline.
- Expiration of obsolete notifications.
- Deep links into the originating application.
- Delivery history.
- Authorized actions.

Events and notifications are not interchangeable. Not every event should interrupt the user.

For example:

| Event | Expected treatment |
|---|---|
| Agent requires permission | Actionable notification |
| Plan requires approval | Actionable notification |
| CI failure | Actionable notification |
| Learn proposes a reusable skill | Review notification or inbox item |
| Execution progress | Activity feed |
| Execution completed | Preference-dependent notification |
| Model escalation | Activity event, potentially notification |

Clicking an approval notification must never itself authorize the underlying action. It should open a validated approval flow with the action, scope, and consequences visible.

## 3.5 Storage, integrations, and security

The platform must provide:

- Shared storage abstractions.
- Per-application namespaces.
- Credential and secret management.
- Integration adapters.
- Access control.
- Data isolation.
- Audit events.
- Schema and migration support.
- Offline event queuing and synchronization where appropriate.

Security must be enforced by the platform, not merely requested by applications.

# 4. Cauldron Code — requirements

Cauldron Code is a specialized agentic development environment running on Cauldron Platform.

It should own the development experience and its development-specific intelligence.

It must not reimplement platform infrastructure.

## 4.1 Application responsibilities

Code owns:

- IDE and workspace experience.
- Project and repository context.
- Agent orchestration.
- Terminal and command execution.
- Git operations and worktrees.
- Development tools and integrations.
- CI and testing workflows.
- Development-specific policies.
- Plan, Execute, Verify, Ship, and Learn workflows.
- Judge and Librarian coordination.
- Decision Engine integration.

Code consumes platform services for model inference, storage, Library access, notifications, and application lifecycle.

## 4.2 Plan gate

The Plan gate translates a user's request into an actionable and testable implementation specification.

For simple, well-defined tasks, Code should generate a one-shot plan.

For ambiguous or high-impact tasks, Code should ask targeted questions before planning. It should support a deeper clarification mode similar to the Grill Me approach.

The output is an authoritative `PLAN.md`.

The plan should contain:

- Problem statement.
- Intended outcome.
- Functional requirements.
- Non-functional requirements.
- Scope and exclusions.
- Relevant project context.
- Architecture and design decisions.
- Acceptance criteria.
- Test strategy.
- Security considerations.
- Implementation sequence.
- Risks and dependencies.
- Expected deliverables.

The plan should be structured enough to drive execution and verification.

The system must distinguish between confirmed requirements, assumptions, and unresolved questions.

## 4.3 Execute gate

Execute turns the approved plan into implementation.

Execution should be model-efficient.

Start with the least expensive compatible model that is likely to succeed. Escalate only when evidence indicates that the current approach is struggling.

Escalation triggers may include:

- Repeated implementation failures.
- Excessive retries.
- Repeatedly incorrect outputs.
- A task that exceeds the current model's capabilities.
- Explicit user dissatisfaction.

Before escalating, diagnose the failure.

A bad result may be caused by an incorrect plan, incomplete context, a faulty tool, or an implementation error—not necessarily a weak model.

When escalating, send a compact failure digest containing:

- Original task and plan.
- Work completed.
- Current code state.
- Failed attempts.
- Relevant tool outputs.
- Test failures.
- Unresolved issues.
- Reason for escalation.

Do not restart the entire task without reason.

Execution must preserve traceability between planned work and actual changes.

## 4.4 Verify gate

Verify determines whether the implementation satisfies the plan.

It must not equate passing tests with successful delivery.

Verification should compare:

- `PLAN.md`.
- `EXECUTE.md`.
- Actual code changes.
- Test results.
- CI results.
- Acceptance criteria.
- Security and dependency findings.
- Deviations and skipped work.

It must identify:

- Requirements not implemented.
- Requirements implemented differently.
- Tests that are missing or insufficient.
- Tests weakened or removed during implementation.
- Security issues.
- Regressions.
- Unjustified scope changes.

Where practical, acceptance tests should be designed before implementation and should fail before the required behavior exists.

Tests must never be weakened merely to obtain a passing result.

Verify should produce an evidence-backed result and a clear list of unresolved issues.

## 4.5 Ship gate

**Ship replaces Learn as the final delivery gate.**

Ship is responsible for deciding whether the verified work is ready to be delivered.

It should consider:

- Whether acceptance criteria are satisfied.
- Whether verification evidence is complete.
- Whether required CI checks pass.
- Whether known risks are acceptable under the applicable policy.
- Whether deployment or release prerequisites are met.
- Whether rollback or recovery procedures exist where needed.
- Whether release notes and relevant documentation are ready.

Ship should support different delivery targets:

- Local project delivery.
- Git commit or branch.
- Pull request.
- Merge.
- Release artifact.
- Deployment.

The target depends on the task and the permissions available.

Ship should produce a delivery record containing:

- What was delivered.
- Where it was delivered.
- Relevant commit or artifact identifiers.
- Verification evidence.
- Remaining known limitations.
- Release or deployment status.

A failed Ship gate should return the work to the appropriate earlier stage. It should not silently mark an incomplete task as delivered.

Routine implementation should proceed autonomously. Human intervention should be reserved for explicit authorization boundaries, irreversible or high-impact actions, missing credentials, genuine ambiguity, or policy requirements—not inserted as a mandatory review step after every gate.

## 4.6 Learn gate — knowledge acquisition

Learn is a distinct knowledge-acquisition stage, not the delivery gate.

Its purpose is to extract useful information from the work performed and make it available to the Librarian.

Learn should be able to operate after execution, verification, shipping, failures, and significant decisions. It should not need to block a successful delivery merely because knowledge curation is still running.

It should inspect:

- The original request and clarified requirements.
- `PLAN.md`.
- Execution history.
- Model and tool behavior.
- Failed and successful attempts.
- Verification evidence.
- Plan deviations.
- Human corrections and overrides.
- Delivery outcome.
- Relevant project changes.
- Existing Library knowledge.

It should identify candidate information such as:

1. **Observations:** What happened?
2. **Decisions:** What was chosen, and why?
3. **Rules:** What constraints or conventions were discovered?
4. **Skills:** What reusable procedure was demonstrated?
5. **Outcomes:** What worked, failed, or produced a measurable result?
6. **Open questions:** What remains uncertain?

Learn must attach provenance to every candidate item.

A candidate should include:

- Source project and task.
- Source artifact or event.
- Timestamp.
- Relevant code or document references.
- Confidence and supporting evidence.
- Scope: task, project, application, or global.
- Proposed resource type.
- Potential conflicts with existing knowledge.

Learn must not automatically turn every conversation, agent message, or transient observation into a global skill.

It should send structured candidate knowledge to the Librarian.

## 4.7 Librarian

The Librarian is responsible for knowledge curation.

It receives candidates from Learn and determines whether they should be retained, updated, rejected, or promoted.

Its responsibilities:

- Validate evidence and provenance.
- Deduplicate information.
- Identify contradictions.
- Determine appropriate scope.
- Distinguish reusable knowledge from project-specific context.
- Version updates.
- Preserve the original source.
- Propose global skill promotion only when justified.
- Avoid turning one-off behavior into a universal rule.

The Librarian should be able to write to the platform Library through its authorized interface.

It should not own the platform's storage or notification infrastructure.

When a significant knowledge promotion requires user awareness, it publishes an event. The platform decides how that event is delivered.

## 4.8 Judge

Judge is distinct from Librarian.

Judge evaluates the quality of decisions and work across the workflow.

It should examine:

- Whether the plan was sound.
- Whether execution followed the plan.
- Whether deviations were justified.
- Whether evidence supports the result.
- Whether verification was sufficient.
- Whether the delivered outcome achieved the intended objective.
- Whether previous decisions should be reconsidered.

Judge evaluates. Librarian curates.

Judge findings can become inputs to Learn and the Librarian, but the two responsibilities must not collapse into one component.

# 5. Decision Engine integration

The Decision Engine should remain a separate repository and reusable software package.

Code bundles it as a dependency.

It is responsible for decision orchestration throughout the Code application, not only one isolated gate.

It should support:

- Typed decisions.
- Decision lifecycle and state transitions.
- Context and evidence gathering through adapters.
- Policy evaluation.
- Model selection and escalation.
- Decision history and event contracts.
- Outcome tracking.
- Human overrides and auditability.
- Typed events for platform services.

The engine should not own:

- Platform model serving.
- Notification delivery.
- Cauldron Library storage.
- Application lifecycle.
- Cauldron-specific UI.
- Cauldron-specific persistence implementations.

The engine defines contracts. Host applications provide implementations.

This allows another application to integrate the engine with its own database, model provider, event system, and policy layer.

The first integration target is Cauldron Code.

# 6. Decision traceability and reproducibility

Every meaningful decision should be reconstructable.

The decision record should preserve:

- Decision identifier.
- Input and requested outcome.
- Relevant context references.
- Evidence considered.
- Model/provider and version.
- Policy version.
- Prompt or structured request version where applicable.
- Decision output.
- Confidence or uncertainty information where applicable.
- Human overrides.
- Execution outcome.
- Subsequent evaluation.

The system must distinguish replaying a stored decision from rerunning a decision using current models or current data.

A model score must never independently authorize sensitive actions. Deterministic policy checks and permission enforcement remain authoritative.

# 7. Current understanding versus target state

The following is a planning baseline based on the project discussions, not a verified audit of the source repositories.

| Area | Current understanding | Target state |
|---|---|---|
| Cauldron architecture | Platform and Code responsibilities need clearer separation | Explicit host platform and application boundaries |
| Model inference | Local and remote model use is part of the vision | Shared platform model runtime and scheduler |
| Model lifecycle | Not yet established as a complete shared service | Registry, loading, unloading, updates, resource scheduling |
| Library | Shared knowledge concept established | Versioned, permissioned platform capability |
| Notifications | Need to be integrated into architecture | Event bus and shared notification service |
| Code workflow | Plan, Execute, Verify concepts established | Formal Plan, Execute, Verify, Ship gates |
| Learn | Previously discussed as part of the workflow | Separate knowledge-acquisition gate |
| Librarian | Knowledge curation role established | Evidence-backed Library curation |
| Judge | Separate evaluation role established | Cross-gate decision and outcome evaluation |
| Decision Engine | Separate reusable repository intended | Standalone library with host adapters |
| App lifecycle | Platform vision established | Install, update, permissions, isolation, compatibility |
| Security | Requirements need formalization | Capability enforcement, auditability, isolation |
| Observability | Needs a unified approach | Cross-platform traces, cost, model, and decision telemetry |

Before implementation, the repository should be audited to replace this planning baseline with a verified file-by-file and feature-by-feature status.

# 8. Autonomous implementation requirements

The objective is a working application, not a collection of architecture documents.

Implementation should proceed in coherent, testable increments without requiring the user to approve every ordinary technical decision.

The autonomous implementation process should:

1. Inspect the existing repositories, code, tests, dependencies, and documentation.
2. Map existing functionality to the target architecture.
3. Identify duplicated responsibilities and dependency violations.
4. Preserve working functionality during separation.
5. Define stable platform interfaces.
6. Move shared infrastructure into the platform where appropriate.
7. Keep Code-specific behavior inside the Code application.
8. Integrate the Decision Engine through explicit adapters.
9. Implement and test the workflow gates.
10. Integrate Learn with Librarian and the shared Library.
11. Implement the event and notification contracts.
12. Add end-to-end tests.
13. Update documentation and developer setup instructions.
14. Verify installation, startup, model execution, app interaction, and delivery workflows.

The implementation must not merely create placeholder modules and declare the architecture complete.

Each capability should be marked as one of:

- Implemented and tested.
- Partially implemented.
- Interface defined, implementation pending.
- Not implemented.
- Blocked by an external dependency.

## 8.1 Autonomy policy

The default mode should be autonomous execution.

Do not introduce mandatory user review at every gate.

The system should make routine implementation choices, run tests, diagnose failures, repair defects, and continue when the evidence supports doing so.

Escalate only when:

- A required permission is unavailable.
- An action is irreversible or materially high-impact.
- A security policy requires explicit authorization.
- External credentials or access are missing.
- Requirements are genuinely ambiguous and cannot be resolved safely.
- The system cannot establish that an operation is safe.

The system must never claim completion without evidence.

## 8.2 Engineering quality requirements

- Preserve existing working functionality.
- Avoid unnecessary rewrites.
- Keep platform and application dependencies one-directional.
- Use explicit interfaces rather than hidden global coupling.
- Add unit, integration, and end-to-end tests.
- Add migration and rollback plans for data changes.
- Preserve audit and decision history.
- Document compatibility requirements.
- Ensure local-only model policies are actually enforced.
- Test offline and degraded-mode behavior where applicable.

# 9. Implementation roadmap

## Phase 0 — Repository audit

- Inspect the current platform and Code repositories.
- Identify the actual application entry points.
- Map shared services and app-specific modules.
- Identify duplicated model, storage, Library, and notification logic.
- Record current tests and known defects.
- Produce a verified current-state architecture.

**Exit condition:** A reliable map of the current implementation and a migration plan.

## Phase 1 — Platform/application boundary

- Establish the platform host boundary.
- Define application manifests and permissions.
- Establish shared service interfaces.
- Move or isolate shared capabilities.
- Ensure Code can run as an application on the platform.

**Exit condition:** Code consumes platform capabilities without the platform depending on Code.

## Phase 2 — Shared runtime

- Implement the model provider abstraction.
- Implement local inference adapters.
- Implement model registry and lifecycle management.
- Implement resource scheduling.
- Connect remote providers.
- Add usage and telemetry reporting.

**Exit condition:** Multiple applications can use the same platform model runtime without owning separate model infrastructure.

## Phase 3 — Shared Library and events

- Establish Library APIs and permissions.
- Implement event contracts and routing.
- Implement notification inbox and delivery.
- Add app-level subscriptions and preferences.
- Integrate Code notifications.

**Exit condition:** Applications can share authorized knowledge and publish/consume platform events.

## Phase 4 — Code workflow

- Formalize Plan.
- Formalize Execute and escalation.
- Formalize Verify and evidence comparison.
- Implement Ship.
- Implement Learn.
- Connect Judge and Librarian.
- Integrate the Decision Engine.

**Exit condition:** A complete development task can move from request to verified delivery, with knowledge capture occurring as a distinct process.

## Phase 5 — Hardening and release

- Security and permission tests.
- End-to-end workflow tests.
- Model scheduling and concurrency tests.
- Recovery and migration tests.
- Performance profiling.
- Installation and update tests.
- Documentation and release packaging.

**Exit condition:** A reproducible, installable, tested platform and Code application.

# 10. Definition of done

The separation is not complete merely because folders or repositories have been renamed.

It is complete when:

1. Cauldron Platform can start independently.
2. Cauldron Code can install and run on the platform.
3. The platform does not depend on Code.
4. Shared model inference is available to multiple applications.
5. Model lifecycle and resource contention are managed centrally.
6. Library access is permissioned and shared.
7. Notifications work through platform events.
8. Code retains its specialized development capabilities.
9. Plan, Execute, Verify, and Ship work end to end.
10. Learn can extract structured knowledge and send it to Librarian.
11. Librarian can curate and publish approved knowledge to Library.
12. Judge can evaluate decisions and outcomes.
13. Decision Engine is independently buildable and integrated as a dependency.
14. Decisions and execution outcomes are traceable.
15. Automated tests cover the critical workflows.
16. The application can be installed, updated, and operated without relying on manual intervention for routine tasks.

# 11. Architectural principle

**Cauldron Platform provides capabilities. Cauldron applications provide experiences and specialized workflows. Decision Engine provides reusable decision orchestration.**

The platform should become more useful as more applications are installed, without becoming coupled to any one application.

Code should be the first application to prove this architecture—not the reason the architecture exists.
