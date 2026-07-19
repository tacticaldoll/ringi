## REMOVED Requirements

### Requirement: A Step's Work Is Performed By A Builder Agent
**Reason**: Ringi no longer performs code-work steps.
**Migration**: Agent CLIs participate as natural-language respondents under `deliberation-loop`.

### Requirement: A Clean Exit Is Success; Anything Else Retries
**Reason**: A clean process exit does not establish a successful deliberation answer or decision.
**Migration**: Process outcome is transport evidence; independent arbitration and humans govern dossier progress.

### Requirement: A Round's Build Work Is Performed By A Builder Agent
**Reason**: Build rounds, workspace edits, and Builder prompts are outside the reframed product.
**Migration**: Use respondent turns with no workspace mutation authority.
