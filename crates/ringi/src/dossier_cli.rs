use crate::dossier::{Frontmatter, LifecycleState, parse_frontmatter, serialize_frontmatter};
use crate::store::DossierStore;
use anyhow::{Context, bail};
use std::path::{Path, PathBuf};

fn dossiers_dir() -> PathBuf {
    Path::new(".ringi").join("dossiers")
}

pub fn draft_command() -> anyhow::Result<()> {
    let dir = dossiers_dir();
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let draft = Frontmatter::new_draft();
    let id = draft.id.to_string();
    let path = dir.join(format!("{}.md", id));

    let content = format!(
        "---\n{}---\n\n# Propose Action Here\n\nWrite your intent...\n",
        serialize_frontmatter(&draft)?
    );

    std::fs::write(&path, content).with_context(|| format!("writing {}", path.display()))?;
    println!("Created draft dossier at {}", path.display());
    Ok(())
}

pub fn submit_command(id: &str, store: &mut DossierStore) -> anyhow::Result<()> {
    let path = dossiers_dir().join(format!("{}.md", id));
    if !path.exists() {
        bail!("Dossier file {} not found", path.display());
    }

    let content = std::fs::read_to_string(&path)?;
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        bail!("Dossier file missing frontmatter");
    }

    let mut frontmatter = parse_frontmatter(parts[1])?;
    if frontmatter.id.to_string() != id {
        bail!("Dossier ID mismatch in frontmatter");
    }

    let submitted = frontmatter
        .clone()
        .submit()
        .map_err(|e| anyhow::anyhow!(e))?;

    // Update the frontmatter state
    frontmatter.state = LifecycleState::Submitted;
    let new_content = format!("---{}---{}", serialize_frontmatter(&frontmatter)?, parts[2]);
    std::fs::write(&path, new_content)?;

    // Commit to SQLite
    let state_json = serde_json::to_string(&submitted)?;
    store.insert_dossier(id, &state_json)?;

    // Create initial revision from the dossier body
    let initial_revision = crate::revision::Revision {
        revision_id: uuid::Uuid::new_v4(),
        parent_digest: None,
        content_digest: crate::revision::Digest("initial-digest".into()),
        original_proposal: parts[2].trim().to_string(),
        current_understanding: parts[2].trim().to_string(),
        positions: vec![],
        dissents: vec![],
        unresolved_risks: vec![],
        readiness: false,
    };
    store.commit_successor_revision(id, None, &initial_revision, &[])?;

    println!("Submitted dossier {}", id);
    Ok(())
}

pub fn decide_command(
    id: &str,
    approve: bool,
    reject: bool,
    store: &DossierStore,
) -> anyhow::Result<()> {
    let state_json = store
        .get_dossier_state(id)?
        .context("Dossier not found in store")?;
    let mut dossier: crate::dossier::SubmittedDossier = serde_json::from_str(&state_json)?;

    if dossier.state != LifecycleState::ReadyForDecision {
        bail!(
            "Dossier is not ready for decision (current state: {:?})",
            dossier.state
        );
    }

    if approve {
        dossier
            .transition_to(LifecycleState::Approved)
            .map_err(|e| anyhow::anyhow!(e))?;
    } else if reject {
        dossier
            .transition_to(LifecycleState::Rejected)
            .map_err(|e| anyhow::anyhow!(e))?;
    } else {
        bail!("Must specify --approve or --reject");
    }

    let new_state_json = serde_json::to_string(&dossier)?;
    store.insert_dossier(id, &new_state_json)?;

    // Update the markdown file too
    let path = dossiers_dir().join(format!("{}.md", id));
    if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() == 3 {
            let mut frontmatter = parse_frontmatter(parts[1])?;
            frontmatter.state = dossier.state;
            let new_content = format!("---{}---{}", serialize_frontmatter(&frontmatter)?, parts[2]);
            std::fs::write(&path, new_content)?;
        }
    }

    println!("Decision recorded for dossier {}: {:?}", id, dossier.state);
    Ok(())
}

pub fn deliberate_command(id: &str, store: &mut DossierStore) -> anyhow::Result<()> {
    let state_json = store
        .get_dossier_state(id)?
        .context("Dossier not found in store")?;
    let mut dossier: crate::dossier::SubmittedDossier = serde_json::from_str(&state_json)?;

    if dossier.state == LifecycleState::Submitted {
        dossier
            .transition_to(LifecycleState::Deliberating)
            .map_err(|e| anyhow::anyhow!(e))?;
        store.insert_dossier(id, &serde_json::to_string(&dossier)?)?;
    }

    crate::deliberate_loop::run_deliberation(id, &state_json, store)
}
