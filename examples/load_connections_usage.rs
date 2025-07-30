/// Example showing how to integrate load_connections into your existing connections_panel.rs
/// 
/// This shows the minimal changes needed to your current code to add connection storage.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::services::{load_connections, save_connections, get_connections_file_path, parse_connection_url, connection_to_url};

// First, make your ConnectionInfo serializable by adding these derive macros:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub hostname: String,
    pub username: String,
    pub password: String,
    pub database: String,
    pub port: String,
}

// Then, in your ConnectionsPanel::new() method, replace the existing spawn block:
/*
OLD CODE:
cx.spawn(async move |view, cx| {
    if let Some(path) = std::env::home_dir() {
        let project_dir = path.join(".pgui");
        let connection_file = project_dir.join("connections.json");
        if connection_file.exists() {
            let connections = load_connections(&connection_file).await;
        }
    }
})
.detach();

NEW CODE:
*/

pub fn example_spawn_in_new() {
    // Inside your ConnectionsPanel::new() method:
    
    // Load saved connections
    let connection_list_entity = connection_list.clone(); // You already have this
    cx.spawn(async move |_view, cx| {
        // Use the helper function to get the correct path
        if let Ok(file_path) = get_connections_file_path() {
            // Load connections from the file
            if let Ok(connections) = load_connections(&file_path).await {
                // Update your connection list delegate with the loaded connections
                connection_list_entity.update(cx, |list, cx| {
                    list.delegate_mut().update_connections(connections);
                    cx.notify();
                }).ok();
            }
        }
    })
    .detach();
}

// Add a method to save the current connection
pub fn save_current_connection_example() {
    // Add this method to your ConnectionsPanel impl:
    
    /*
    pub fn save_current_connection(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let connection_url = self.input_esc.read(cx).value().clone();
        
        if connection_url.trim().is_empty() {
            return;
        }

        let connection_list_entity = self.connection_list.clone();
        cx.spawn(async move |_this, cx| {
            // Parse the connection URL into ConnectionInfo
            if let Ok(connection_info) = parse_connection_url(&connection_url) {
                // Get the connections file path
                if let Ok(file_path) = get_connections_file_path() {
                    // Load existing connections
                    if let Ok(mut connections) = load_connections(&file_path).await {
                        // Check if connection already exists
                        let exists = connections.iter().any(|conn| {
                            conn.hostname == connection_info.hostname 
                                && conn.username == connection_info.username 
                                && conn.database == connection_info.database
                                && conn.port == connection_info.port
                        });

                        if !exists {
                            connections.push(connection_info);
                            // Save back to file
                            if let Ok(()) = save_connections(&file_path, &connections).await {
                                // Update the UI list
                                connection_list_entity.update(cx, |list, cx| {
                                    list.delegate_mut().update_connections(connections);
                                    cx.notify();
                                }).ok();
                            }
                        }
                    }
                }
            }
        })
        .detach();
    }
    */
}

// Update your connection button section to include a save button:
pub fn render_connection_section_example() {
    /*
    fn render_connection_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let connection_button = if self.is_connected {
            Button::new("disconnect")
                .label("Disconnect")
                .icon(Icon::empty().path("icons/unplug.svg"))
                .small()
                .danger()
                .on_click(cx.listener(Self::disconnect_from_database))
        } else {
            Button::new("connect")
                .label(if self.is_loading { "Connecting..." } else { "Connect" })
                .icon(Icon::empty().path("icons/plug-zap.svg"))
                .small()
                .outline()
                .disabled(self.is_loading)
                .on_click(cx.listener(Self::connect_to_database))
        };

        // Add a save button
        let save_button = Button::new("save_connection")
            .label("Save")
            .icon(Icon::empty().path("icons/save.svg"))
            .small()
            .ghost()
            .on_click(cx.listener(Self::save_current_connection));

        v_flex()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(Label::new("Database Connection").font_bold().text_sm())
            .child(TextInput::new(&self.input_esc).cleanable())
            .child(
                h_flex()
                    .gap_2()
                    .child(connection_button)
                    .child(save_button)  // Add the save button here
            )
    }
    */
}

// Update your ListEvent::Confirm handler to use the new connection_to_url function
pub fn list_event_confirm_example() {
    /*
    cx.subscribe_in(
        &connection_list,
        window,
        |this, _, ev: &ListEvent, window, cx| match ev {
            ListEvent::Confirm(ix) => {
                if let Some(conn) = this.get_selected_connection(*ix, cx) {
                    // Use the helper function to convert ConnectionInfo to URL
                    let con_str = connection_to_url(&conn);
                    this.input_esc.update(cx, |is, cx| {
                        is.set_value(con_str, window, cx);
                        cx.notify();
                    })
                }
            }
            _ => {}
        },
    )
    */
}

// The JSON file format will look like this:
/*
{
  "connections": [
    {
      "hostname": "localhost",
      "username": "postgres",
      "password": "mypassword",
      "database": "mydb",
      "port": "5432"
    },
    {
      "hostname": "production.example.com",
      "username": "appuser",
      "password": "secretpassword",
      "database": "production_db",
      "port": "5432"
    }
  ],
  "version": "1.0"
}
*/

// Quick integration guide:
/*
1. Add these imports to your connections_panel.rs:
   use serde::{Deserialize, Serialize};
   use crate::services::{load_connections, save_connections, get_connections_file_path, parse_connection_url, connection_to_url};

2. Add Serialize, Deserialize derives to your ConnectionInfo struct

3. Replace the spawn block in new() with the example above

4. Add the save_current_connection method

5. Update your render_connection_section to include the save button

6. Update your ListEvent::Confirm handler to use connection_to_url

That's it! Your connections will now be automatically saved to ~/.pgui/connections.json
*/