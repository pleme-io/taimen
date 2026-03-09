//! Rhai scripting integration for Taimen.
//!
//! Enables server-side automation scripts for video conferencing management.
//! Loads scripts from a configurable directory and exposes room management
//! functions for automated workflows.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use soushi::ScriptEngine;

/// Script hook events that can trigger server-side scripts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptEvent {
    /// A room was created.
    RoomCreated { room_id: String, name: String },
    /// A room ended.
    RoomEnded { room_id: String },
    /// A participant joined a room.
    ParticipantJoined { room_id: String, user: String },
    /// A participant left a room.
    ParticipantLeft { room_id: String, user: String },
    /// Server started.
    ServerStarted,
}

/// Manages the Rhai scripting engine with taimen-specific server functions.
pub struct TaimenScripting {
    engine: ScriptEngine,
    /// Compiled event hook scripts (ASTs keyed by event name).
    hooks: std::collections::HashMap<String, soushi::rhai::AST>,
}

impl TaimenScripting {
    /// Create a new scripting engine with taimen room management functions.
    ///
    /// Registers: `taimen.create_room(name)`, `taimen.end_room(id)`,
    /// `taimen.list_rooms()`.
    ///
    /// The `action_tx` is used to queue actions for the server to process.
    #[must_use]
    pub fn new(action_tx: Arc<Mutex<Vec<ScriptAction>>>) -> Self {
        let mut engine = ScriptEngine::new();
        engine.register_builtin_log();
        engine.register_builtin_env();
        engine.register_builtin_string();

        // taimen.create_room(name)
        let tx = action_tx.clone();
        engine.register_fn("taimen_create_room", move |name: &str| {
            if let Ok(mut actions) = tx.lock() {
                actions.push(ScriptAction::CreateRoom(name.to_string()));
            }
        });

        // taimen.end_room(id)
        let tx = action_tx.clone();
        engine.register_fn("taimen_end_room", move |id: &str| {
            if let Ok(mut actions) = tx.lock() {
                actions.push(ScriptAction::EndRoom(id.to_string()));
            }
        });

        // taimen.list_rooms()
        let tx = action_tx;
        engine.register_fn("taimen_list_rooms", move || -> String {
            if let Ok(mut actions) = tx.lock() {
                actions.push(ScriptAction::ListRooms);
            }
            String::new()
        });

        Self {
            engine,
            hooks: std::collections::HashMap::new(),
        }
    }

    /// Load all scripts from the given directory.
    pub fn load_scripts_from(&mut self, dir: &std::path::Path) -> Result<Vec<String>, soushi::SoushiError> {
        if !dir.is_dir() {
            tracing::debug!(path = %dir.display(), "scripts directory not found, skipping");
            return Ok(Vec::new());
        }
        self.engine.load_scripts_dir(dir)
    }

    /// Load scripts from the default config directory.
    ///
    /// Looks in `~/.config/taimen/scripts/` by default.
    pub fn load_scripts(&mut self) -> Result<Vec<String>, soushi::SoushiError> {
        let scripts_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("taimen")
            .join("scripts");
        self.load_scripts_from(&scripts_dir)
    }

    /// Register an event hook script.
    pub fn register_hook(&mut self, event_name: &str, script: &str) -> Result<(), soushi::SoushiError> {
        let ast = self.engine.compile(script)?;
        self.hooks.insert(event_name.to_string(), ast);
        Ok(())
    }

    /// Fire an event, running any registered hook scripts.
    pub fn fire_event(&self, event: &ScriptEvent) {
        let event_name = match event {
            ScriptEvent::RoomCreated { .. } => "room_created",
            ScriptEvent::RoomEnded { .. } => "room_ended",
            ScriptEvent::ParticipantJoined { .. } => "participant_joined",
            ScriptEvent::ParticipantLeft { .. } => "participant_left",
            ScriptEvent::ServerStarted => "server_started",
        };

        if let Some(ast) = self.hooks.get(event_name) {
            if let Err(e) = self.engine.eval_ast(ast) {
                tracing::error!(event = event_name, error = %e, "script hook failed");
            }
        }
    }

    /// Evaluate an ad-hoc script string.
    pub fn eval(&self, script: &str) -> Result<soushi::rhai::Dynamic, soushi::SoushiError> {
        self.engine.eval(script)
    }
}

/// Actions that scripts can request from the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptAction {
    /// Create a new room with the given name.
    CreateRoom(String),
    /// End a room by ID.
    EndRoom(String),
    /// Request the room list.
    ListRooms,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> (TaimenScripting, Arc<Mutex<Vec<ScriptAction>>>) {
        let actions = Arc::new(Mutex::new(Vec::new()));
        let engine = TaimenScripting::new(actions.clone());
        (engine, actions)
    }

    #[test]
    fn create_room_function_queues_action() {
        let (engine, actions) = make_engine();
        engine
            .eval(r#"taimen_create_room("standup")"#)
            .unwrap();
        let actions = actions.lock().unwrap();
        assert_eq!(actions[0], ScriptAction::CreateRoom("standup".to_string()));
    }

    #[test]
    fn end_room_function_queues_action() {
        let (engine, actions) = make_engine();
        engine.eval(r#"taimen_end_room("room-123")"#).unwrap();
        let actions = actions.lock().unwrap();
        assert_eq!(actions[0], ScriptAction::EndRoom("room-123".to_string()));
    }

    #[test]
    fn list_rooms_function_queues_action() {
        let (engine, actions) = make_engine();
        engine.eval("taimen_list_rooms()").unwrap();
        let actions = actions.lock().unwrap();
        assert_eq!(actions[0], ScriptAction::ListRooms);
    }

    #[test]
    fn fire_event_with_no_hook_is_noop() {
        let (engine, _actions) = make_engine();
        engine.fire_event(&ScriptEvent::ServerStarted);
    }

    #[test]
    fn register_and_fire_hook() {
        let (mut engine, actions) = make_engine();
        engine
            .register_hook("server_started", r#"taimen_create_room("lobby")"#)
            .unwrap();
        engine.fire_event(&ScriptEvent::ServerStarted);
        let actions = actions.lock().unwrap();
        assert_eq!(actions[0], ScriptAction::CreateRoom("lobby".to_string()));
    }

    #[test]
    fn load_scripts_missing_dir_returns_empty() {
        let (mut engine, _actions) = make_engine();
        let result = engine.load_scripts();
        assert!(result.is_ok());
    }

    #[test]
    fn eval_arbitrary_script() {
        let (engine, _actions) = make_engine();
        let result = engine.eval("40 + 2").unwrap();
        assert_eq!(result.as_int().unwrap(), 42);
    }
}
