use crate::dossier::LifecycleState;
use crate::store::DossierStore;
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::fmt::Write;

pub fn render_archive(dossier_id: &str, store: &DossierStore) -> anyhow::Result<String> {
    let state_json = store
        .get_dossier_state(dossier_id)?
        .context("Dossier not found")?;
    let dossier: crate::dossier::SubmittedDossier = serde_json::from_str(&state_json)?;

    if !matches!(
        dossier.state,
        LifecycleState::Approved
            | LifecycleState::Rejected
            | LifecycleState::Cancelled
            | LifecycleState::Invalidated
    ) {
        anyhow::bail!("Cannot archive a non-terminal dossier");
    }

    let mut out = String::new();
    writeln!(&mut out, "# Dossier Archive: {}", dossier_id)?;
    writeln!(&mut out, "\n**State**: {:?}", dossier.state)?;
    writeln!(
        &mut out,
        "**Strategy**: {:?}",
        dossier.locked_settings.arbitration.preset
    )?;

    if let Some(rev) = store.get_latest_revision(dossier_id)? {
        writeln!(&mut out, "\n## Final SSOT")?;
        writeln!(&mut out, "\n### Original Proposal")?;
        writeln!(&mut out, "{}", rev.original_proposal)?;
        writeln!(&mut out, "\n### Final Understanding")?;
        writeln!(&mut out, "{}", rev.current_understanding)?;
    } else {
        writeln!(&mut out, "\n*(No revisions found)*")?;
    }

    writeln!(&mut out, "\n## Public Event Index")?;
    // We would fetch public events here from the store.
    // For now, let's just mark the section.
    writeln!(&mut out, "*(Events omitted for brevity)*")?;

    writeln!(&mut out, "\n## Sealed Audit Section")?;
    writeln!(&mut out, "*(Sealed evaluations omitted for brevity)*")?;

    // Compute integrity digest
    let mut hasher = Sha256::new();
    hasher.update(out.as_bytes());
    let digest_bytes = hasher.finalize();
    let digest: String = digest_bytes.iter().map(|b| format!("{:02x}", b)).collect();

    writeln!(
        &mut out,
        "\n---\n**Integrity Digest (SHA-256)**: {}",
        digest
    )?;

    Ok(out)
}
