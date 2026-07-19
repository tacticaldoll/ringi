## REMOVED Requirements

### Requirement: A Reviewer Agent Produces Findings Through The Agent Seam
**Reason**: Code-review findings are replaced by proposal positions, dissent, and risks.
**Migration**: Configure independent respondent and arbitration roles under `deliberation-loop`.

### Requirement: Reviewer Output Is A Quality Opinion, Never A Permission
**Reason**: The specific Reviewer role is removed, though its no-permission invariant remains.
**Migration**: No respondent, arbitrator, evaluator, confidence signal, or readiness judgment can grant human approval.
