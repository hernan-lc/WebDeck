use crate::{
    contracts::*,
    domain::{self, Error, Result},
    server::App,
};
pub const REMOTE: &[Capability] = &[
    Capability::Read,
    Capability::Input,
    Capability::Audio,
    Capability::Window,
    Capability::Power,
    Capability::Script,
    Capability::Network,
    Capability::Plugin,
];
pub fn recommended(app: &App) -> Result<Vec<Capability>> {
    let snapshot = app.config.snapshot()?;
    let mut needed = vec![Capability::Read];
    for button in snapshot
        .config
        .layout
        .folders
        .iter()
        .flat_map(|f| &f.buttons)
    {
        if let Some(command) = domain::action_command(&button.action) {
            if let Err(error) = collect(app, &command, &mut needed, 0) {
                // An unavailable plugin or cyclic button reference must not prevent pairing.
                if error.code != ErrorCode::InvalidInput {
                    return Err(error);
                }
            }
        }
    }
    needed.retain(|cap| REMOTE.contains(cap));
    needed.sort_by_key(|cap| REMOTE.iter().position(|c| c == cap));
    needed.dedup();
    Ok(needed)
}
fn collect(app: &App, command: &Command, needed: &mut Vec<Capability>, depth: usize) -> Result<()> {
    if depth > 32 {
        return Err(Error::invalid());
    }
    needed.push(command.capability());
    match command {
        Command::Button { button_id } => {
            let snapshot = app.config.snapshot()?;
            let command = snapshot
                .config
                .layout
                .folders
                .iter()
                .flat_map(|f| &f.buttons)
                .find(|b| &b.id == button_id)
                .and_then(|b| domain::action_command(&b.action))
                .ok_or_else(Error::invalid)?;
            collect(app, &command, needed, depth + 1)?;
        }
        Command::Workflow { workflow } => node(app, workflow, needed, depth + 1)?,
        Command::Plugin {
            plugin_id,
            action_id,
            ..
        } => {
            let action = app
                .plugins
                .iter()
                .find(|p| &p.id == plugin_id)
                .and_then(|p| p.actions.iter().find(|a| &a.id == action_id))
                .ok_or_else(Error::invalid)?;
            needed.extend(&action.capabilities);
        }
        _ => {}
    }
    Ok(())
}
fn node(
    app: &App,
    workflow: &WorkflowNode,
    needed: &mut Vec<Capability>,
    depth: usize,
) -> Result<()> {
    if depth > 32 {
        return Err(Error::invalid());
    }
    match workflow {
        WorkflowNode::Command { command, .. } => collect(app, command, needed, depth + 1)?,
        WorkflowNode::Sequence { steps } | WorkflowNode::Parallel { steps } => {
            for step in steps {
                node(app, step, needed, depth + 1)?;
            }
        }
        WorkflowNode::Conditional {
            if_true, if_false, ..
        } => {
            node(app, if_true, needed, depth + 1)?;
            node(app, if_false, needed, depth + 1)?;
        }
        WorkflowNode::Retry { step, .. } | WorkflowNode::Timeout { step, .. } => {
            node(app, step, needed, depth + 1)?
        }
        WorkflowNode::Script { .. } => needed.push(Capability::Script),
        WorkflowNode::Plugin {
            plugin_id,
            version,
            action_id,
            args,
        } => collect(
            app,
            &Command::Plugin {
                plugin_id: plugin_id.clone(),
                version: version.clone(),
                action_id: action_id.clone(),
                args: args.clone(),
            },
            needed,
            depth + 1,
        )?,
        _ => {}
    }
    Ok(())
}
