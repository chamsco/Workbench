# M0 audit: Cauldron against PLAN.md

Date: 2026-10-30. Audited commit: `1d508e6` (origin/main, PR #25). Unmerged
branches (PRs #27 to #37: model server, harness relay, agent hands, shell rail)
are not included.

Status rules: **works** needs a passing test that covers the PLAN.md
behaviour, not just a related one. **partial** means code and tests exist for
part of it. **missing** means no code found. **unknown** means code exists but
no test proves it.

Suite at the audited commit (`cargo test --no-fail-fast`, default members:
runner, core, cli, colony): **204 passed, 0 failed, 0 ignored**. Not in the
suite: `crates/tauri-app` (UI, 4 tests, needs GTK libs), `crates/app` (GPUI,
does not link here: missing libxkbcommon), `capsule/` (Go agentd + its own
Rust core). Full output is at the end.

## Section 5 features

| Feature | Status | Evidence |
|---|---|---|
| Chat box intake that starts a project or task | partial | `Harness::send` starts a project (tests/e2e.rs `flat_project_with_gates_escalation_and_merges`, `a_ticket_runs_on_a_cli_worker`). No intake step that hands a task to triage. |
| Local decision model triage (one-shot / long / research + confidence, ask below threshold) | missing | `router.rs` picks (model, effort) per agent with a confidence (Jev API or keyword heuristic, `heuristic_scales_with_task`). Local triage exists only for filed out-of-scope tickets (features.rs `triage_can_run_on_the_local_model_with_no_api_key`, e2e `filed_tickets_get_triaged`). No task-type answer, no threshold, no ask-the-user path. |
| Planning mode (grill-me interview) | partial | `spec.rs` gate + vendored `grilling` skill; tests: 6 in spec.rs, tests/spec_gate.rs, e2e `a_plan_must_cover_the_spec`, 5 in phase.rs. Its checklist is Problem / User stories / Acceptance / Open questions. Missing vs section 6: one test per criterion, non-goals as a rule, constraints, dependencies, "before" baseline. No human "yes" gate on the spec. |
| Markdown handoff (spec.md, tests.md) | partial | `SPEC.md` written by the app (spec.rs `file`), tickets mirrored to `.backspace/tickets/*.md` (ticket.rs `parses_and_renders`). No tests.md, no failure digest file. |
| Builder and Rampart orchestrators | missing | No code. Nothing seals or hash-locks tests; no fail-on-baseline, flake or mutation check. |
| Frontier orchestrator only when needed, small/local executors | partial | Workers start cheap and climb on evidence (`escalate` in harness.rs, e2e flat project test). Local model routing tested (features.rs `a_worker_routes_to_the_local_model_only_while_it_is_listening`, `a_delegated_lookup_reaches_the_chosen_local_model`). But the main agent always runs on its configured model, escalation allows `max_steps: 3` climbs (PLAN says ONE frontier call with a digest), and there is no cost-aware escalation. |
| Automatic PR creation | missing | Accepted work is merged locally into the parent branch (git.rs `branch_merge_and_conflict`). No PR is opened. |
| Readiness check, send-back loop, CI/CD, before/after judging | partial | A ticket's `check` command gates review and a rejected deliverable goes back (e2e flat project). No readiness score, no different-model reviewer rule, no 3-loop cap test, no CI/CD step, no before/after comparison (`show_options` is a user picker, not a judge). |
| Per-project allowlists (models, skills, plugins; separate orchestration/execution lists) | missing | Only tool deny rules exist (mcp.rs `rules_keep_tools_away`, agents.rs `rules_become_denied_tools`). No model, skill or plugin allowlist; no single gateway. |
| Per-task token/cost budgets at the gateway | partial | `budget_usd` per ticket and project, turn budget per effort level (harness.rs `over_budget`); e2e only asserts the budget appears in the brief. Not enforced at a gateway, no test that spending stops. |
| Master memory with a memory decision agent | partial | Git-versioned notes with revert and scopes (memory.rs, 5 tests); a logistic capture model that learns from rejections (memory_learn.rs, 3 tests); `looks_secret` filter. Missing: proposal schema (type, evidence, source, ttl), evidence check, duplicate/contradiction check, global-promotion approval, eval. **Conflict:** memory stores notes of kind `"instruction"`; section 8.3 says entries are data, never instructions. |
| herdr adapter | missing | No code. |
| Plugin and skill system | partial | Skills load on demand (skills.rs, 5 tests). No plugin API, no audit hook, no skill/plugin allowlist. (`ui/extensions` are UI apps, not the 3.2 plugin API.) |
| In-app auto-testing and regression, ADE and IDE | missing | No affected-test selection, no sandboxed run on edit, no regression block. Ticket `check` is a single command run by an agent. |
| Continuous chat with threads, shared context, cloud and local execution | partial (scope unconfirmed) | chat.rs threads (12 tests), cloud.rs `dev_server_end_to_end`, e2e `a_remote_machine_is_followed_and_driven`. PLAN marks this "confirm scope". |

## Cross-cutting items PLAN.md requires

| Item | Status | Evidence |
|---|---|---|
| Section 3.4 metrics per merged PR | partial | trace.rs records tokens per span (OTel, 2 tests). No per-PR frontier-call, cost-to-green or escalation-rate metrics. |
| Sandboxed execution (worktree + container) | partial | Worktree per ticket (e2e, git.rs). No container isolation for test runs. |
| Router eval from real traces | missing | No eval set or report. |
| Replay suite (recorded traces, stubbed models, frozen router) | partial | e2e.rs runs the harness against a scripted fake model server; not recorded traces. |

## Blockers and open questions (I need your answer)

1. **Feature list (section 5 TODO).** M0 acceptance says "Feature list complete". The TODO is still there, so M0 cannot pass as written. Paste the remaining features, or tell me the list above is final.
2. **Language (Q1: TypeScript or Go).** The repo is Rust: ~23k lines in `crates/core`, 204 passing tests, plus a Go `capsule/agentd`. Neither option matches. I recommend Rust for the orchestrator so it extends `harness.rs`, `router.rs`, `spec.rs` and `memory.rs` instead of rewriting them. Switching to TS or Go means a rewrite and discarding these tests. Your call.
3. **herdr release (Q3).** "v2" not found; blocks M6 only.
4. **Decision model (Q4).** The code already calls Jev over an API with a keyword fallback. PLAN wants a local model behind a swappable interface. Which candidate do I start with for M2? (Blocks M2, not M1.)
5. **Features that exist but are not in section 5**: routines, delegate board, apps, voice, iOS companion, colony, memory auto-capture. Do I leave them untouched (my default), or are they in scope for the audit and later milestones?
6. **Memory "instruction" notes** contradict section 8.3. M10 would have to stop storing or injecting them. Confirm.
7. **Naming.** Code and config call the app "Backspace" (`~/.config/backspace`, crate names). PLAN calls it Cauldron/Workbench. I will not rename anything unless you ask.
8. **PLAN.md was not in the repo.** I committed your attached copy at the root. It is now the source of truth.

## Cost for M0

Product code made 0 model calls. This session runs on a frontier model; I cannot read its exact token count, so I am not reporting a number I don't have.

## Full suite output

```
Running unittests src/main.rs (target/debug/deps/backspace_cli-e14ffda91c75b4ab)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/lib.rs (target/debug/deps/backspace_colony-0abb1bf7f3d72512)

running 25 tests
test backend::tests::cli_states_fold_into_five ... ok
test backend::tests::capabilities_print_providers ... ok
test backend::tests::cli_workspace_json_maps_to_ours ... ok
test backend::tests::create_args_are_the_cli_flags ... ok
test backend::tests::error_message_comes_out_verbatim ... ok
test backend::tests::a_missing_cli_is_an_error_not_a_panic ... ok
test backend::tests::exec_output_reads_the_cli_exit_code ... ok
test backend::tests::parse_error_ignores_noise_and_junk ... ok
test colony::tests::a_corrupt_state_file_is_an_error ... ok
test colony::tests::colony_json_round_trips ... ok
test colony::tests::a_missing_state_file_is_an_empty_colony ... ok
test colony::tests::duplicate_name_is_refused_before_the_backend ... ok
test colony::tests::exec_records_argv_and_returns_the_output ... ok
test colony::tests::list_adopts_what_the_backend_reports ... ok
test colony::tests::task_and_created_survive_a_backend_round_trip ... ok
test colony::tests::unknown_workspace_is_an_error_not_a_panic ... ok
test fake::tests::the_fake_refuses_a_duplicate_like_the_cli ... ok
test colony::tests::stop_force_reaches_the_backend ... ok
test fake::tests::the_fake_reports_running_then_stopped ... ok
test prompt::tests::chat_prompt_copes_without_a_task ... ok
test prompt::tests::chat_prompt_names_the_workspace ... ok
test tools::tests::five_tools_with_the_right_names ... ok
test tools::tests::every_tool_carries_a_description_and_an_object_schema ... ok
test tools::tests::required_arguments_are_the_ones_the_registry_needs ... ok
test colony::tests::lifecycle_create_list_status_stop_start_delete ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running unittests src/lib.rs (target/debug/deps/backspace_core-506ff0b8f481c1a9)

running 140 tests
test accounts::tests::default_profile_dir_sits_under_the_data_accounts_folder ... ok
test accounts::tests::env_for_uses_the_providers_own_variable ... ok
test accounts::tests::best_prefers_healthy_over_warning_at_equal_failures ... ok
test accounts::tests::order_drops_disabled_and_cooled_down_accounts ... ok
test accounts::tests::note_limit_cools_down_and_note_fail_does_not ... ok
test accounts::tests::order_ranks_state_then_failures_then_priority_then_last_ok ... ok
test accounts::tests::policies_decide_who_switches ... ok
test accounts::tests::fails_decay_after_a_day_and_the_ring_stays_small ... ok
test accounts::tests::round_trips_through_json_and_decays_on_load ... ok
test agents::tests::next_run_round_trips_a_negative_offset ... ok
test agents::tests::quoting ... ok
test agents::tests::next_run_maths ... ok
test agents::tests::routine_summaries ... ok
test agents::tests::rules_become_denied_tools ... ok
test agents::tests::brief_in_names_the_project_personality_and_routines ... ok
test agents::tests::save_cleans_routines_and_refreshes_a_passed_wake ... ok
test agents::tests::routine_save_validates_and_persists ... ok
test apps::tests::manifest_rules ... ok
test artifact::tests::a_body_cannot_be_unbounded_and_a_summary_says_what_it_is ... ok
test artifact::tests::a_table_row_must_match_the_header ... ok
test artifact::tests::an_old_state_file_without_artifacts_still_loads ... ok
test agents::tests::save_list_brief_delete ... ok
test artifact::tests::each_kind_is_validated_on_its_own_field ... ok
test artifact::tests::ids_are_unique_and_addressable ... ok
test artifact::tests::options_must_refer_to_a_set_that_exists ... ok
test artifact::tests::kinds_this_build_does_not_know_are_kept_not_dropped ... ok
test artifact::tests::the_list_is_bounded_and_drops_the_oldest ... ok
test board::tests::a_thread_is_the_pair_whichever_way_it_went ... ok
test board::tests::conversations_group_replies_and_broadcasts_separately ... ok
test board::tests::post_and_read ... ok
test apps::tests::install_serve_store_remove ... ok
test chat::tests::a_thread_file_without_a_life_loads_as_a_normal_chat ... ok
test chat::tests::ansi_is_stripped ... ok
test chat::tests::base64_roundtrip ... ok
test chat::tests::link_meta ... ok
test chat::tests::making_a_thread_a_ghost_deletes_the_file_it_had ... ok
test chat::tests::mentions_pick_a_cli ... ok
test chat::tests::a_ghost_thread_is_never_written_and_never_listed ... ok
test chat::tests::a_temporal_thread_forgets_itself_after_its_deadline ... ok
test chat::tests::purging_ghosts_removes_only_ghosts ... ok
test chat::tests::titles_are_short ... ok
test agents::tests::docker_computer ... ok
test cloud::tests::plan_table ... ok
test companion::tests::pair_link_escapes ... ok
test chat::tests::talking_to_a_temporal_thread_moves_its_deadline ... ok
test config::tests::default_config_parses ... ok
test delegate::tests::a_task_whose_bot_is_gone_comes_back_unassigned ... ok
test delegate::tests::an_empty_title_is_refused ... ok
test delegate::tests::auto_off_leaves_work_unassigned ... ok
test delegate::tests::adds_assigns_and_survives_a_reopen ... ok
test delegate::tests::importing_tickets_twice_keeps_one_task_each ... ok
test delegate::tests::auto_picks_the_shortest_queue_then_the_cheaper_minute ... ok
test diagram::tests::plan_layers_by_blockers_and_flags_missing_checks ... ok
test effort::tests::clamps_up_then_down ... ok
test delegate::tests::the_snapshot_prices_a_queue_and_a_cli_is_free ... ok
test git::tests::a_new_repo_lists_folders_and_says_so ... ok
test git::tests::day_numbers ... ok
test git::tests::parse_changes_reads_status_and_counts ... ok
test harnesses::tests::codex_plan_from_id_token ... ok
test harnesses::tests::toggles_override_scan ... ok
test harnesses::tests::versions ... ok
test layout::tests::a_card_can_come_out_and_a_split_still_works ... ok
test layout::tests::a_full_stack_never_loses_a_canvas ... ok
test layout::tests::a_stack_survives_json_and_older_shapes_still_load ... ok
test layout::tests::centre_swaps_and_self_is_a_no_op ... ok
test layout::tests::closing_a_card_keeps_the_stack_valid_and_renumbers_it ... ok
test layout::tests::defaults_and_presets_cover_every_pane ... ok
test layout::tests::drag_to_an_edge_splits_there_and_the_old_spot_closes ... ok
test layout::tests::dropping_on_a_cards_middle_stacks_instead_of_swapping ... ok
test layout::tests::json_shape ... ok
test layout::tests::one_over_two_is_a_long_top_and_two_squares ... ok
test layout::tests::remove_and_renumber_keep_it_valid ... ok
test layout::tests::stacking_two_keeps_both_and_raising_swaps_which_is_in_front ... ok
test layout::tests::zones ... ok
test mcp::tests::rules_keep_tools_away ... ok
test memory::tests::dates_and_meta ... ok
test chat::tests::threads_persist_react_branch_delete ... ok
test memory::tests::moves_old_json_in ... ok
test cloud::tests::dev_server_end_to_end ... ok
test memory::tests::repo_format_hand_edits_and_history ... ok
test memory_dream::tests::plan_applies_conservatively ... ok
test memory_dream::tests::prompt_lists_notes_and_messages ... ok
test memory_learn::tests::learns_from_rejections ... ok
test memory_learn::tests::proposes_and_skips ... ok
test memory_learn::tests::third_person_rewrites ... ok
test model_server::tests::human_size_reads_in_decimal_units ... ok
test model_server::tests::is_loopback_only_for_this_machine ... ok
test model_server::tests::openai_base_adds_v1_for_ollama ... ok
test model_server::tests::order_breaks_ties_by_size_then_id ... ok
test model_server::tests::order_puts_the_smallest_tiny_first ... ok
test model_server::tests::parse_ollama_tags_fixture ... ok
test model_server::tests::parse_openai_models_fixture ... ok
test model_server::tests::pick_falls_back_and_handles_empty ... ok
test model_server::tests::pick_maps_the_task_kind ... ok
test model_server::tests::pick_prefers_the_named_family ... ok
test model_server::tests::recommend_needs_a_running_runtime ... ok
test model_server::tests::recommend_prefers_the_tiny_model_and_stays_stable ... ok
test model_server::tests::tiny_by_parameter_count ... ok
test model_server::tests::tiny_from_the_file_size_only_when_nothing_else_says ... ok
test model_server::tests::tiny_from_the_name_when_the_count_is_missing ... ok
test options::tests::add_names_variants_and_pick_records_the_choice ... ok
test options::tests::bad_sets_are_refused ... ok
test phase::tests::a_model_can_block_but_never_open ... ok
test phase::tests::a_refusal_is_kept_and_a_pass_moves_on ... ok
test phase::tests::a_refused_respec_reopens_specify_and_a_pass_does_not_skip_ahead ... ok
test phase::tests::loads_and_saves_beside_the_project ... ok
test phase::tests::round_trips_and_old_state_loads_without_it ... ok
test projects::tests::save_list_rename_assign_delete ... ok
test provider::tests::anthropic_body_sets_effort_only_when_supported ... ok
test provider::tests::openai_roundtrips_tool_calls ... ok
test router::liveness_tests::a_listening_port_is_up_and_a_closed_one_is_not ... ok
test memory::tests::scopes_and_persistence ... ok
test router::tests::heuristic_scales_with_task ... ok
test skills::tests::a_workspace_folder_overrides_a_default_and_shows_as_ours ... ok
test skills::tests::defaults_load_with_descriptions ... ok
test skills::tests::description_reads_plain_folded_and_literal ... ok
test skills::tests::found_says_where_a_skill_came_from ... ok
test skills::tests::search_ranks_a_name_hit_above_a_body_hit ... ok
test spec::tests::a_complete_spec_passes ... ok
test spec::tests::coverage_names_every_hole ... ok
test spec::tests::each_rule_names_what_is_missing ... ok
test spec::tests::extract_takes_the_last_spec_block_only ... ok
test spec::tests::the_prompt_carries_the_grilling_skill_the_rules_and_the_last_refusal ... ok
test spec::tests::the_skeleton_alone_is_refused_for_every_empty_part ... ok
test ticket::tests::parses_and_renders ... ok
test tools::tests::resolve_blocks_escape ... ok
test tools::tests::write_edit_read_bash ... ok
test memory::tests::the_librarian_files_a_batch_as_one_commit ... ok
test trace::tests::records_tools_and_exports_otlp ... ok
test update::tests::a_merged_pull_request_reads_as_its_title ... ok
test update::tests::another_commit_is_newer_but_the_same_one_is_not ... ok
test update::tests::falls_back_to_the_notes ... ok
test update::tests::notes_without_a_stamp_say_nothing ... ok
test update::tests::only_a_hash_counts_as_a_commit ... ok
test update::tests::reads_the_commit_the_workflow_passed ... ok
test update::tests::the_changelog_is_pull_requests_newest_first ... ok
test update::tests::the_stamp_is_the_commit_the_build_was_given ... ok
test router::tests::cli_pin_routes_workers_not_the_planner ... ok
test trace::tests::exports_to_a_collector ... ok
test git::tests::branch_merge_and_conflict ... ok

test result: ok. 140 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.59s

     Running tests/agents.rs (target/debug/deps/agents-3e3bf92970288c0d)

running 1 test
test groups_take_turns_and_agents_work_in_their_folder ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s

     Running tests/bya.rs (target/debug/deps/bya-e3043d92b9939e25)

running 1 test
test chat_with_a_remote_a2a_agent ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests/e2e.rs (target/debug/deps/e2e-36d83ee279d41137)

running 7 tests
test filed_tickets_get_triaged ... ok
test a_plan_must_cover_the_spec ... ok
test a_ticket_runs_on_a_cli_worker ... ok
test a_remote_machine_is_followed_and_driven ... ok
test visual_options_wait_for_the_users_pick ... ok
test flat_project_with_gates_escalation_and_merges ... ok
test nested_leads_review_and_revise_their_reports ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s

     Running tests/features.rs (target/debug/deps/features-f89239a6df61383c)

running 17 tests
test a_limit_is_recognised_from_the_clis_own_words_only ... ok
test a_direct_message_between_two_agents_is_its_own_conversation ... ok
test a_local_model_on_a_custom_router_is_found_by_id ... ok
test a_chosen_local_model_becomes_the_scout_model ... ok
test a_reply_to_a_message_that_is_not_there_is_refused ... ok
test accounts_round_trip_and_keep_their_env ... ok
test each_project_remembers_its_own_canvases ... ok
test local_jobs_defaults_to_scouts_and_refuses_unknown_words ... ok
test no_choice_or_an_unknown_runtime_leaves_the_config_alone ... ok
test one_limited_login_does_not_stop_the_project ... ok
test read_messages_hands_back_the_id_a_reply_can_name ... ok
test the_board_message_shape_survives_a_round_trip ... ok
test the_tick_gates_the_local_model_and_scouts_only_follow_their_own ... ok
test a_worker_routes_to_the_local_model_only_while_it_is_listening ... ok
test a_produced_artifact_reaches_the_snapshot_and_is_addressable ... ok
test a_delegated_lookup_reaches_the_chosen_local_model ... ok
test triage_can_run_on_the_local_model_with_no_api_key ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s

     Running tests/interrupted.rs (target/debug/deps/interrupted-f51eb1a30baed879)

running 2 tests
test only_an_interrupted_agent_can_carry_on ... ok
test a_restart_carries_a_cli_worker_on_in_its_own_session ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s

     Running tests/resume.rs (target/debug/deps/resume-c3b5cae2affceb3b)

running 1 test
test interrupted_replies_resume ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests/spec_gate.rs (target/debug/deps/spec_gate-94498de447b56d3b)

running 1 test
test the_gate_refuses_twice_then_opens ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s

     Running tests/webhook.rs (target/debug/deps/webhook-7da8c2b7530ee62e)

running 2 tests
test the_phone_cannot_read_agent_or_group_threads ... ok
test a_webhook_wakes_only_its_bot ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

     Running unittests src/lib.rs (target/debug/deps/backspace_runner-7f3ebfbbcbbefa7b)

running 3 tests
test tests::transcripts_and_base64 ... ok
test tests::claude_stream_with_a_tool_call ... ok
test tests::codex_stream ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/http.rs (target/debug/deps/http-425a39f3299f7e6e)

running 3 tests
test openai_compatible_stream ... ok
test bad_agent_addresses_say_so ... ok
test a2a_card_and_task ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.54s

   Doc-tests backspace_colony

running 1 test
test crates/colony/src/lib.rs - (line 15) - compile ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

   Doc-tests backspace_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests backspace_runner

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
