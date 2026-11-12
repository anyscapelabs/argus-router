<!-- BEGIN:turborepo-agent-rules -->

# This is NOT the Turborepo you know

Turborepo configuration, task behavior, and CLI commands can vary between installed versions and may differ from your training data. Resolve the `turbo` package from this file's directory or relevant workspace; in monorepos, it may not be visible from the repository root. For example, run `node -p "require.resolve('turbo/package.json')"` from a workspace that depends on `turbo`.

Read `docs/README.md` inside that installed package first, then read the relevant pages from its `docs/` directory before changing Turborepo configuration or commands. Heed deprecation notices. These bundled docs match the installed package version and are available without network access.

This block is written and re-added by `turbo` before repository-scoped commands when an AI agent is detected. In the Turborepo source repository, its template is defined in `crates/turborepo-cli/src/cli/agent_guidance.rs`. Removing the managed block while updates are enabled means a later qualifying invocation will add it again. Set `"agentGuidance": true` in the root `turbo.json` or `turbo.jsonc` to opt in; this does not remove an existing block. Keep the block committed with your work to avoid an uncommitted change on the next agent invocation.
<!-- END:turborepo-agent-rules -->

# Engineering Standards

Follow these rules for every implementation.

## 1. Understand before implementing
- Inspect the existing codebase before making changes.
- Understand existing architecture, conventions and data flow.
- Identify the actual problem before proposing a solution.
- Do not assume how existing components work.

## 2. Simplicity
- Prefer the simplest correct implementation.
- Avoid unnecessary abstractions, interfaces and design patterns.
- Do not introduce dependencies without justification.
- Avoid overengineering and premature optimization.
- Prefer readable code over clever code.

## 3. Code organization
- Keep functions focused on one responsibility.
- Keep business logic separate from infrastructure.
- Avoid unnecessary global state.
- Minimize side effects.
- Use meaningful names for variables, functions and types.
- Avoid duplicated business logic.

## 4. Reliability and security
- Handle errors explicitly.
- Validate untrusted input at system boundaries.
- Never silently ignore important errors.
- Consider concurrency, race conditions and resource cleanup.
- Follow least-privilege security principles.

## 5. Testing
- Write tests for important behaviour.
- Include failure cases and edge cases.
- Run relevant tests after implementation.
- Never claim tests passed without actually running them.
- Never treat a successful compilation as proof of correctness.

## 6. Modification discipline
- Make focused, incremental changes.
- Do not modify unrelated files.
- Preserve existing functionality unless explicitly instructed otherwise.
- Review the complete Git diff before finishing.
- Remove unused code introduced by the implementation.

## 7. Performance
- Prioritize correctness before optimization.
- Measure before optimizing.
- Avoid unnecessary allocations, database queries and network requests.
- Consider resource consumption and scalability.

## 8. Final review
Before completing a task:
- Verify the implementation.
- Review the code for unnecessary complexity.
- Check for security and reliability issues.
- Confirm tests and builds.
- Explain what changed and any remaining limitations.

**Core principle: Write the simplest maintainable code that correctly solves the actual problem.**

# Comments

Do not add comments unless they explain something non-obvious that the code itself cannot say (a load-bearing "why"). Do not write comments that restate what the code does. Keep the codebase comment-free by default.

