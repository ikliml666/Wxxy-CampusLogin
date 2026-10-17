===

压缩对话范围：m01175–m01238 (CodeWiki 更新)

TASK AS OF THIS BLOCK:
- **Goal**: Rewrite `learnings/windows-hello-gate-module-singleton.md` based on current source code (`selfServiceState.ts`, `AccountPanel.tsx`, `OnboardingWizard.tsx`). Preserve structure, title, type, tags.
- **Findings**:
    - **No Design Flaw**: The previous article claimed a "design flaw" where the binding gate state leaked between `AccountPanel` and `OnboardingWizard`. Current code comments (`selfServiceState.ts:14-19`) explicitly state this is intentional ("与自助服务面板的会话门完全独立...首次操作即验证，通过后 TTL 内共用"). It behaves like a sudo timestamp.
    - **Gate Logic**:
        - `useHelloGate()`: Binding gate. Uses `helloGateVerifiedAt`. Supports `ignoreToggle` (for password reveal). TTL = 570s. Handles clock drift.
        - `useSelfServiceVerify()`: Self-service session gate. Uses `selfSessionVerifiedAt`. Respects `config.selfReverifyEachAction`. Reset via `resetSelfSessionGate()`.
    - **Usage**:
        - `AccountPanel.tsx`: Delegates to `useOperatorBind` hook (not inline logic as previously described). `handleReveal` uses `ignoreToggle: true`.
        - `OnboardingWizard.tsx`: Uses `useHelloGate()` for binding step.
- **Actions**: Generated new article content matching these findings. Outputted full markdown.

[ACP] Compressed m01175-m01238 CodeWiki 更新