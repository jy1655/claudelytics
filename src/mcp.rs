//! Model Context Protocol (MCP) server implementation for claudelytics
//!
//! Provides MCP resources and tools for accessing Claude usage analytics data
//! through a standardized protocol that other applications can consume.

use crate::parser::UsageParser;
use crate::reports::{
    SortField as ReportSortField, SortOrder as ReportSortOrder, generate_daily_report_sorted,
    generate_monthly_report_sorted, generate_session_report_sorted,
};
use anyhow::{Context, Result};
use chrono::Local;
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;

/// MCP server for claudelytics data access
pub struct McpServer {
    claude_path: PathBuf,
}

/// MCP Resource definition
#[derive(Debug, Clone)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}

/// MCP Tool definition
#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl McpServer {
    /// Create a new MCP server instance
    pub fn new(claude_path: PathBuf) -> Self {
        Self { claude_path }
    }

    /// Run MCP server over stdio with JSON-RPC framing
    pub fn run_stdio(&self) -> Result<()> {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        let stdout = io::stdout();
        let mut writer = stdout.lock();

        loop {
            let request = match Self::read_framed_message(&mut reader)? {
                Some(request) => request,
                None => break, // EOF
            };

            if let Some(response) = self.handle_request(request) {
                Self::write_framed_message(&mut writer, &response)?;
            }
        }

        Ok(())
    }

    fn read_framed_message<R: BufRead + Read>(reader: &mut R) -> Result<Option<Value>> {
        let mut content_length: Option<usize> = None;

        loop {
            let mut line = String::new();
            let bytes = reader.read_line(&mut line)?;

            if bytes == 0 {
                return Ok(None); // EOF
            }

            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }

            if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                let parsed = value
                    .trim()
                    .parse::<usize>()
                    .context("Invalid Content-Length header")?;
                content_length = Some(parsed);
            }
        }

        let length = content_length
            .ok_or_else(|| anyhow::anyhow!("Missing Content-Length in MCP request"))?;

        let mut payload = vec![0_u8; length];
        reader.read_exact(&mut payload)?;

        let request: Value =
            serde_json::from_slice(&payload).context("Invalid JSON payload in MCP request")?;
        Ok(Some(request))
    }

    fn write_framed_message<W: Write>(writer: &mut W, message: &Value) -> Result<()> {
        let payload = serde_json::to_vec(message)?;
        write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
        writer.write_all(&payload)?;
        writer.flush()?;
        Ok(())
    }

    fn handle_request(&self, request: Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

        // Notification (no response required)
        let id = id?;

        if method == "initialize" {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "listChanged": false }
                    },
                    "serverInfo": {
                        "name": "claudelytics",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }));
        }

        if method == "ping" {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            }));
        }

        let result = match method {
            "tools/list" => Ok(self.tools_list_response()),
            "resources/list" => Ok(self.resources_list_response()),
            "resources/read" => self.handle_resources_read(&params),
            "tools/call" => self.handle_tool_call(&params),
            _ => {
                return Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method not found: {}", method)
                    }
                }));
            }
        };

        Some(match result {
            Ok(value) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": value
            }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32000,
                    "message": e.to_string()
                }
            }),
        })
    }

    fn tools_list_response(&self) -> Value {
        let tools = self
            .list_tools()
            .into_iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema
                })
            })
            .collect::<Vec<_>>();
        json!({ "tools": tools })
    }

    fn resources_list_response(&self) -> Value {
        let resources = self
            .list_resources()
            .into_iter()
            .map(|resource| {
                json!({
                    "uri": resource.uri,
                    "name": resource.name,
                    "description": resource.description,
                    "mimeType": resource.mime_type
                })
            })
            .collect::<Vec<_>>();
        json!({ "resources": resources })
    }

    fn handle_resources_read(&self, params: &Value) -> Result<Value> {
        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("resources/read requires 'uri'"))?;

        let data = self.read_resource(uri)?;
        Ok(json!({
            "contents": [{
                "uri": uri,
                "mimeType": "application/json",
                "text": serde_json::to_string_pretty(&data)?
            }]
        }))
    }

    fn handle_tool_call(&self, params: &Value) -> Result<Value> {
        let tool_name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("tools/call requires 'name'"))?;
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let data = match tool_name {
            "get_usage_data" => self.tool_get_usage_data(&arguments)?,
            "get_cost_summary" => self.tool_get_cost_summary(&arguments)?,
            "find_sessions" => self.tool_find_sessions(&arguments)?,
            _ => anyhow::bail!("Unknown tool: {}", tool_name),
        };

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&data)?
            }],
            "isError": false
        }))
    }

    fn tool_get_usage_data(&self, args: &Value) -> Result<Value> {
        let report_type = args
            .get("report_type")
            .and_then(|v| v.as_str())
            .unwrap_or("daily");
        let since = args
            .get("since")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let until = args
            .get("until")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let sort_field = parse_sort_field(args.get("sort_field").and_then(|v| v.as_str()));
        let sort_order = parse_sort_order(args.get("sort_order").and_then(|v| v.as_str()));

        let parser = UsageParser::new(self.claude_path.clone(), since, until, None)?;
        let (daily_map, session_map, _) = parser.parse_all()?;

        let report = match report_type {
            "daily" => serde_json::to_value(generate_daily_report_sorted(
                daily_map, sort_field, sort_order,
            ))?,
            "session" => serde_json::to_value(generate_session_report_sorted(
                session_map,
                sort_field,
                sort_order,
            ))?,
            "monthly" => serde_json::to_value(generate_monthly_report_sorted(
                daily_map, sort_field, sort_order,
            ))?,
            _ => anyhow::bail!("Invalid report_type: {}", report_type),
        };

        Ok(report)
    }

    fn tool_get_cost_summary(&self, args: &Value) -> Result<Value> {
        let parser = UsageParser::new(self.claude_path.clone(), None, None, None)?;
        let (daily_map, _, _) = parser.parse_all()?;
        let daily_report = generate_daily_report_sorted(daily_map, None, None);

        if let Some(date) = args.get("date").and_then(|v| v.as_str()) {
            if date == "today" {
                let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
                if let Some(day) = daily_report.daily.iter().find(|d| d.date == today) {
                    return Ok(json!({
                        "date": day.date,
                        "total_cost": day.total_cost,
                        "total_tokens": day.total_tokens
                    }));
                }
                return Ok(json!({
                    "date": today,
                    "total_cost": 0.0,
                    "total_tokens": 0
                }));
            }

            if date.len() == 8 {
                let formatted = format!("{}-{}-{}", &date[0..4], &date[4..6], &date[6..8]);
                if let Some(day) = daily_report.daily.iter().find(|d| d.date == formatted) {
                    return Ok(json!({
                        "date": day.date,
                        "total_cost": day.total_cost,
                        "total_tokens": day.total_tokens
                    }));
                }
                return Ok(json!({
                    "date": formatted,
                    "total_cost": 0.0,
                    "total_tokens": 0
                }));
            }

            anyhow::bail!("date must be YYYYMMDD or 'today'");
        }

        Ok(json!({
            "total_cost": daily_report.totals.total_cost,
            "total_tokens": daily_report.totals.total_tokens,
            "days_with_usage": daily_report.daily.len(),
            "latest_usage": daily_report.daily.first().map(|d| json!({
                "date": d.date,
                "total_cost": d.total_cost,
                "total_tokens": d.total_tokens
            }))
        }))
    }

    fn tool_find_sessions(&self, args: &Value) -> Result<Value> {
        let parser = UsageParser::new(self.claude_path.clone(), None, None, None)?;
        let (_, session_map, _) = parser.parse_all()?;

        let project_filter = args.get("project_filter").and_then(|v| v.as_str());
        let min_cost = args.get("min_cost").and_then(|v| v.as_f64());
        let max_cost = args.get("max_cost").and_then(|v| v.as_f64());
        let min_tokens = args.get("min_tokens").and_then(|v| v.as_u64());
        let date_start = args
            .get("date_range")
            .and_then(|v| v.get("start"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let date_end = args
            .get("date_range")
            .and_then(|v| v.get("end"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut sessions = session_map
            .into_iter()
            .filter_map(|(session_path, (usage, last_activity))| {
                if let Some(filter) = project_filter
                    && !session_path.contains(filter)
                {
                    return None;
                }

                if let Some(min) = min_cost
                    && usage.total_cost < min
                {
                    return None;
                }

                if let Some(max) = max_cost
                    && usage.total_cost > max
                {
                    return None;
                }

                if let Some(min) = min_tokens
                    && usage.total_tokens() < min
                {
                    return None;
                }

                let date_yyyymmdd = last_activity.format("%Y%m%d").to_string();
                if let Some(start) = &date_start
                    && date_yyyymmdd < *start
                {
                    return None;
                }
                if let Some(end) = &date_end
                    && date_yyyymmdd > *end
                {
                    return None;
                }

                let parts: Vec<&str> = session_path.split('/').collect();
                let session_id = parts.last().copied().unwrap_or("unknown").to_string();
                let project_path = if parts.len() > 1 {
                    parts[..parts.len() - 1].join("/")
                } else {
                    String::new()
                };

                Some(json!({
                    "session_path": session_path,
                    "project_path": project_path,
                    "session_id": session_id,
                    "last_activity": last_activity.to_rfc3339(),
                    "total_tokens": usage.total_tokens(),
                    "total_cost": usage.total_cost,
                    "input_tokens": usage.input_tokens,
                    "output_tokens": usage.output_tokens,
                    "cache_creation_tokens": usage.cache_creation_tokens,
                    "cache_read_tokens": usage.cache_read_tokens
                }))
            })
            .collect::<Vec<_>>();

        sessions.sort_by(|a, b| {
            let a_last = a["last_activity"].as_str().unwrap_or("");
            let b_last = b["last_activity"].as_str().unwrap_or("");
            b_last.cmp(a_last)
        });

        Ok(json!({ "sessions": sessions }))
    }

    fn read_resource(&self, uri: &str) -> Result<Value> {
        let parser = UsageParser::new(self.claude_path.clone(), None, None, None)?;
        let (daily_map, session_map, _) = parser.parse_all()?;

        match uri {
            "claudelytics://daily-usage" => Ok(serde_json::to_value(
                generate_daily_report_sorted(daily_map, None, None),
            )?),
            "claudelytics://session-usage" => Ok(serde_json::to_value(
                generate_session_report_sorted(session_map, None, None),
            )?),
            "claudelytics://monthly-usage" => Ok(serde_json::to_value(
                generate_monthly_report_sorted(daily_map, None, None),
            )?),
            "claudelytics://cost-summary" => {
                let report = generate_daily_report_sorted(daily_map, None, None);
                Ok(json!({
                    "total_cost": report.totals.total_cost,
                    "total_tokens": report.totals.total_tokens,
                    "days_with_usage": report.daily.len()
                }))
            }
            _ => anyhow::bail!("Unknown resource URI: {}", uri),
        }
    }

    /// Get list of available MCP resources
    pub fn list_resources(&self) -> Vec<McpResource> {
        vec![
            McpResource {
                uri: "claudelytics://daily-usage".to_string(),
                name: "daily-usage".to_string(),
                description: "Daily Claude usage aggregated by date".to_string(),
                mime_type: "application/json".to_string(),
            },
            McpResource {
                uri: "claudelytics://session-usage".to_string(),
                name: "session-usage".to_string(),
                description: "Claude usage aggregated by sessions".to_string(),
                mime_type: "application/json".to_string(),
            },
            McpResource {
                uri: "claudelytics://monthly-usage".to_string(),
                name: "monthly-usage".to_string(),
                description: "Claude usage aggregated by month".to_string(),
                mime_type: "application/json".to_string(),
            },
            McpResource {
                uri: "claudelytics://cost-summary".to_string(),
                name: "cost-summary".to_string(),
                description: "Total cost summary and statistics".to_string(),
                mime_type: "application/json".to_string(),
            },
        ]
    }

    /// Get list of available MCP tools
    pub fn list_tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "get_usage_data".to_string(),
                description: "Get Claude usage data with optional filtering and sorting"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "report_type": {
                            "type": "string",
                            "enum": ["daily", "session", "monthly"],
                            "description": "Type of report to generate"
                        },
                        "since": {
                            "type": "string",
                            "description": "Start date in YYYYMMDD format",
                            "pattern": "^\\d{8}$"
                        },
                        "until": {
                            "type": "string",
                            "description": "End date in YYYYMMDD format",
                            "pattern": "^\\d{8}$"
                        },
                        "sort_field": {
                            "type": "string",
                            "enum": ["date", "cost", "tokens", "efficiency", "project"],
                            "description": "Field to sort by"
                        },
                        "sort_order": {
                            "type": "string",
                            "enum": ["asc", "desc"],
                            "description": "Sort order"
                        }
                    },
                    "required": ["report_type"]
                }),
            },
            McpTool {
                name: "get_cost_summary".to_string(),
                description: "Get cost summary for a specific date or total".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "date": {
                            "type": "string",
                            "description": "Specific date in YYYYMMDD format, or 'today' for today",
                            "pattern": "^(\\d{8}|today)$"
                        }
                    }
                }),
            },
            McpTool {
                name: "find_sessions".to_string(),
                description: "Find sessions matching specific criteria".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "project_filter": {
                            "type": "string",
                            "description": "Filter sessions by project path (supports regex)"
                        },
                        "min_cost": {
                            "type": "number",
                            "description": "Minimum cost threshold"
                        },
                        "max_cost": {
                            "type": "number",
                            "description": "Maximum cost threshold"
                        },
                        "min_tokens": {
                            "type": "integer",
                            "description": "Minimum token count"
                        },
                        "date_range": {
                            "type": "object",
                            "properties": {
                                "start": {"type": "string", "pattern": "^\\d{8}$"},
                                "end": {"type": "string", "pattern": "^\\d{8}$"}
                            }
                        }
                    }
                }),
            },
        ]
    }
}

/// MCP server capability advertisement
pub fn get_server_info() -> Value {
    json!({
        "name": "claudelytics",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Claude Code usage analytics via Model Context Protocol",
        "author": "jy1655 (forked from nwiizo/claudelytics)",
        "homepage": "https://github.com/jy1655/claudelytics",
        "upstream": "https://github.com/nwiizo/claudelytics",
        "capabilities": {
            "resources": true,
            "tools": true,
            "prompts": false
        },
        "protocolVersion": "1.0.0"
    })
}

fn parse_sort_field(value: Option<&str>) -> Option<ReportSortField> {
    match value {
        Some("date") => Some(ReportSortField::Date),
        Some("cost") => Some(ReportSortField::Cost),
        Some("tokens") => Some(ReportSortField::Tokens),
        Some("efficiency") => Some(ReportSortField::Efficiency),
        Some("project") => Some(ReportSortField::Project),
        _ => None,
    }
}

fn parse_sort_order(value: Option<&str>) -> Option<ReportSortOrder> {
    match value {
        Some("asc") => Some(ReportSortOrder::Asc),
        Some("desc") => Some(ReportSortOrder::Desc),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_mcp_server_creation() {
        let server = McpServer::new(PathBuf::from("/tmp"));

        assert_eq!(server.list_resources().len(), 4);
        assert_eq!(server.list_tools().len(), 3);
    }

    #[test]
    fn test_server_info() {
        let info = get_server_info();
        assert_eq!(info["name"], "claudelytics");
        assert_eq!(info["protocolVersion"], "1.0.0");
    }
}
