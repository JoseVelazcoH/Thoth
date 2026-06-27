use crate::prompt::{prompt_snippet, PromptFramework};

#[derive(Debug, Clone, PartialEq)]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct Check {
    pub status: CheckStatus,
    pub name: String,
    pub guidance: Option<String>,
}

#[derive(Debug)]
pub struct DoctorReport {
    pub checks: Vec<Check>,
}

pub struct DoctorInputs {
    pub hooks_installed: bool,
    pub db_result: Result<DbInfo, String>,
    pub tth_on_path: bool,
    pub framework: PromptFramework,
    pub framework_config_text: Option<String>,
}

pub struct DbInfo {
    pub schema_version: i64,
    pub total_commands: i64,
    pub last_timestamp: Option<i64>,
}

pub fn run_doctor(inputs: &DoctorInputs) -> DoctorReport {
    let mut checks = Vec::new();

    if inputs.hooks_installed {
        checks.push(Check {
            status: CheckStatus::Ok,
            name: "hooks installed".to_string(),
            guidance: None,
        });
    } else {
        checks.push(Check {
            status: CheckStatus::Fail,
            name: "hooks installed".to_string(),
            guidance: Some("Run: tth install".to_string()),
        });
    }

    match &inputs.db_result {
        Ok(info) => {
            let detail = format!(
                "schema {}, {} commands, last {}",
                info.schema_version,
                info.total_commands,
                info.last_timestamp
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".to_string())
            );
            checks.push(Check {
                status: CheckStatus::Ok,
                name: format!("database  {detail}"),
                guidance: None,
            });
        }
        Err(msg) => {
            checks.push(Check {
                status: CheckStatus::Fail,
                name: "database".to_string(),
                guidance: Some(format!("DB error: {msg}")),
            });
        }
    }

    if inputs.tth_on_path {
        checks.push(Check {
            status: CheckStatus::Ok,
            name: "tth on PATH".to_string(),
            guidance: None,
        });
    } else {
        checks.push(Check {
            status: CheckStatus::Warn,
            name: "tth on PATH".to_string(),
            guidance: Some("tth not found on PATH; ensure your shell rc is loaded".to_string()),
        });
    }

    let prompt_check =
        check_prompt_visibility(&inputs.framework, inputs.framework_config_text.as_deref());
    checks.push(prompt_check);

    DoctorReport { checks }
}

fn check_prompt_visibility(framework: &PromptFramework, config_text: Option<&str>) -> Check {
    match framework {
        PromptFramework::Generic => Check {
            status: CheckStatus::Warn,
            name: "prompt tags visibility".to_string(),
            guidance: Some(format!(
                "Cannot introspect PROMPT/PS1 automatically.\n{}",
                prompt_snippet(framework)
            )),
        },
        _ => {
            let configured = config_text
                .map(|text| text.contains("thoth_tags") || text.contains("TTH_PROMPT_TAGS"))
                .unwrap_or(false);
            if configured {
                Check {
                    status: CheckStatus::Ok,
                    name: "prompt tags visibility".to_string(),
                    guidance: None,
                }
            } else {
                Check {
                    status: CheckStatus::Warn,
                    name: "prompt tags visibility".to_string(),
                    guidance: Some(format!(
                        "TTH_PROMPT_TAGS not referenced in framework config. Add:\n{}",
                        prompt_snippet(framework)
                    )),
                }
            }
        }
    }
}

pub fn render_report(report: &DoctorReport) -> String {
    let mut out = String::new();
    for check in &report.checks {
        let tag = match check.status {
            CheckStatus::Ok => "[ok]  ",
            CheckStatus::Warn => "[warn]",
            CheckStatus::Fail => "[fail]",
        };
        out.push_str(&format!("{tag} {}\n", check.name));
        if let Some(ref guidance) = check.guidance {
            for line in guidance.lines() {
                out.push_str(&format!("       -> {line}\n"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_inputs(
        hooks: bool,
        db: bool,
        tth: bool,
        framework: PromptFramework,
        config: Option<&str>,
    ) -> DoctorInputs {
        DoctorInputs {
            hooks_installed: hooks,
            db_result: if db {
                Ok(DbInfo {
                    schema_version: 3,
                    total_commands: 42,
                    last_timestamp: Some(1700000000),
                })
            } else {
                Err("connection failed".to_string())
            },
            tth_on_path: tth,
            framework,
            framework_config_text: config.map(|s| s.to_string()),
        }
    }

    #[test]
    fn hooks_ok_when_installed() {
        let inputs = make_inputs(true, true, true, PromptFramework::Generic, None);
        let report = run_doctor(&inputs);
        assert_eq!(report.checks[0].status, CheckStatus::Ok);
        assert!(report.checks[0].name.contains("hooks"));
    }

    #[test]
    fn hooks_fail_when_not_installed() {
        let inputs = make_inputs(false, true, true, PromptFramework::Generic, None);
        let report = run_doctor(&inputs);
        assert_eq!(report.checks[0].status, CheckStatus::Fail);
        let guidance = report.checks[0].guidance.as_deref().unwrap_or("");
        assert!(guidance.contains("tth install"));
    }

    #[test]
    fn db_ok_when_accessible() {
        let inputs = make_inputs(true, true, true, PromptFramework::Generic, None);
        let report = run_doctor(&inputs);
        assert_eq!(report.checks[1].status, CheckStatus::Ok);
        assert!(report.checks[1].name.contains("schema 3"));
        assert!(report.checks[1].name.contains("42 commands"));
    }

    #[test]
    fn db_fail_when_error() {
        let inputs = make_inputs(true, false, true, PromptFramework::Generic, None);
        let report = run_doctor(&inputs);
        assert_eq!(report.checks[1].status, CheckStatus::Fail);
    }

    #[test]
    fn tth_ok_when_on_path() {
        let inputs = make_inputs(true, true, true, PromptFramework::Generic, None);
        let report = run_doctor(&inputs);
        assert_eq!(report.checks[2].status, CheckStatus::Ok);
    }

    #[test]
    fn tth_warn_when_not_on_path() {
        let inputs = make_inputs(true, true, false, PromptFramework::Generic, None);
        let report = run_doctor(&inputs);
        assert_eq!(report.checks[2].status, CheckStatus::Warn);
    }

    #[test]
    fn prompt_ok_when_starship_config_has_thoth_tags() {
        let config = "[env_var.thoth_tags]\nvariable = \"TTH_PROMPT_TAGS\"";
        let inputs = make_inputs(true, true, true, PromptFramework::Starship, Some(config));
        let report = run_doctor(&inputs);
        let prompt_check = report.checks.last().unwrap();
        assert_eq!(prompt_check.status, CheckStatus::Ok);
    }

    #[test]
    fn prompt_warn_when_starship_config_lacks_thoth_tags() {
        let config = "[character]\nsuccess_symbol = \"[>](bold green)\"";
        let inputs = make_inputs(true, true, true, PromptFramework::Starship, Some(config));
        let report = run_doctor(&inputs);
        let prompt_check = report.checks.last().unwrap();
        assert_eq!(prompt_check.status, CheckStatus::Warn);
        let guidance = prompt_check.guidance.as_deref().unwrap_or("");
        assert!(guidance.contains("env_var.thoth_tags"));
    }

    #[test]
    fn prompt_warn_for_generic_is_advisory() {
        let inputs = make_inputs(true, true, true, PromptFramework::Generic, None);
        let report = run_doctor(&inputs);
        let prompt_check = report.checks.last().unwrap();
        assert_eq!(prompt_check.status, CheckStatus::Warn);
    }

    #[test]
    fn render_report_contains_ok_fail_warn() {
        let report = DoctorReport {
            checks: vec![
                Check {
                    status: CheckStatus::Ok,
                    name: "alpha".to_string(),
                    guidance: None,
                },
                Check {
                    status: CheckStatus::Warn,
                    name: "beta".to_string(),
                    guidance: Some("fix it".to_string()),
                },
                Check {
                    status: CheckStatus::Fail,
                    name: "gamma".to_string(),
                    guidance: Some("run cmd".to_string()),
                },
            ],
        };
        let output = render_report(&report);
        assert!(output.contains("[ok]"));
        assert!(output.contains("[warn]"));
        assert!(output.contains("[fail]"));
        assert!(output.contains("-> fix it"));
        assert!(output.contains("-> run cmd"));
    }
}
