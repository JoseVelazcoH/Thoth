use std::path::Path;

const DOCS_ANCHOR: &str = "https://github.com/JoseVelazcoH/thoth#prompt-setup";

#[derive(Debug, Clone, PartialEq)]
pub enum PromptFramework {
    Starship,
    Powerlevel10k,
    OhMyPosh,
    Generic,
}

pub struct DetectInputs {
    pub starship_toml_exists: bool,
    pub starship_on_path: bool,
    pub zsh_theme: Option<String>,
    pub p10k_zsh_exists: bool,
    pub oh_my_posh_dir_exists: bool,
    pub posh_theme_set: bool,
}

pub fn detect_framework(inputs: &DetectInputs) -> PromptFramework {
    if inputs.starship_toml_exists || inputs.starship_on_path {
        return PromptFramework::Starship;
    }
    let p10k_theme = inputs
        .zsh_theme
        .as_deref()
        .map(|t| t.to_ascii_lowercase().contains("powerlevel10k"))
        .unwrap_or(false);
    if p10k_theme || inputs.p10k_zsh_exists {
        return PromptFramework::Powerlevel10k;
    }
    if inputs.oh_my_posh_dir_exists || inputs.posh_theme_set {
        return PromptFramework::OhMyPosh;
    }
    PromptFramework::Generic
}

pub fn probe_inputs() -> DetectInputs {
    let home = std::env::var("HOME").unwrap_or_default();
    let home_path = Path::new(&home);

    let starship_toml_exists = home_path.join(".config/starship.toml").exists();
    let starship_on_path = path_has("starship");
    let zsh_theme = std::env::var("ZSH_THEME").ok();
    let p10k_zsh_exists = home_path.join(".p10k.zsh").exists();
    let oh_my_posh_dir_exists = home_path.join(".config/oh-my-posh").exists();
    let posh_theme_set = std::env::var("POSH_THEME").is_ok();

    DetectInputs {
        starship_toml_exists,
        starship_on_path,
        zsh_theme,
        p10k_zsh_exists,
        oh_my_posh_dir_exists,
        posh_theme_set,
    }
}

fn path_has(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).exists()))
        .unwrap_or(false)
}

pub fn prompt_snippet(framework: &PromptFramework) -> String {
    match framework {
        PromptFramework::Starship => format!(
            "# Add to ~/.config/starship.toml:\n\
             [env_var.thoth_tags]\n\
             variable = \"TTH_PROMPT_TAGS\"\n\
             format = \"[$env_value]($style) \"\n\
             style = \"bold yellow\"\n\
             \n\
             # IMPORTANT: add ${{env_var.thoth_tags}} to your top-level format string.\n\
             # Starship does not render modules that are not referenced in format.\n\
             # Example: format = \"$git_status ${{env_var.thoth_tags}} $character\"\n\
             \n\
             # Docs: {DOCS_ANCHOR}"
        ),
        PromptFramework::Powerlevel10k => format!(
            "# Add to your ~/.zshrc (after p10k is loaded):\n\
             \n\
             # 1. Define a custom segment function:\n\
             prompt_tth_tags() {{\n\
               p10k segment -t \"$TTH_PROMPT_TAGS\"\n\
             }}\n\
             \n\
             # 2. Add tth_tags to your prompt elements, e.g.:\n\
             # POWERLEVEL9K_LEFT_PROMPT_ELEMENTS=(... tth_tags)\n\
             # or POWERLEVEL9K_RIGHT_PROMPT_ELEMENTS=(... tth_tags)\n\
             \n\
             # See: https://github.com/romkatv/powerlevel10k#batteries-included\n\
             # Docs: {DOCS_ANCHOR}"
        ),
        PromptFramework::OhMyPosh => format!(
            "# Add a segment to your oh-my-posh theme JSON/YAML:\n\
             # JSON example (type: text segment reading TTH_PROMPT_TAGS):\n\
             # {{\n\
             #   \"type\": \"text\",\n\
             #   \"template\": \"{{{{ .Env.TTH_PROMPT_TAGS }}}}\"\n\
             # }}\n\
             \n\
             # YAML example:\n\
             # - type: text\n\
             #   template: \"{{{{ .Env.TTH_PROMPT_TAGS }}}}\"\n\
             \n\
             # See: https://ohmyposh.dev/docs/segments/system/text\n\
             # Docs: {DOCS_ANCHOR}"
        ),
        PromptFramework::Generic => format!(
            "# Add to your shell rc file:\n\
             # zsh: PROMPT=\"${{TTH_PROMPT_TAGS}} $PROMPT\"\n\
             # bash: PS1=\"${{TTH_PROMPT_TAGS}} $PS1\"\n\
             \n\
             # Add ${{TTH_PROMPT_TAGS}} to your PROMPT (zsh) or PS1 (bash) where you want active tags to show.\n\
             \n\
             # Docs: {DOCS_ANCHOR}"
        ),
    }
}

pub fn parse_framework(s: &str) -> Result<PromptFramework, crate::error::ThothError> {
    match s.to_ascii_lowercase().as_str() {
        "starship" => Ok(PromptFramework::Starship),
        "powerlevel10k" | "p10k" => Ok(PromptFramework::Powerlevel10k),
        "oh-my-posh" | "ohmyposh" | "omp" => Ok(PromptFramework::OhMyPosh),
        "generic" => Ok(PromptFramework::Generic),
        other => Err(crate::error::ThothError::Prompt(format!(
            "unknown framework '{other}'; use starship, powerlevel10k, oh-my-posh, or generic"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(
        starship_toml: bool,
        starship_path: bool,
        zsh_theme: Option<&str>,
        p10k: bool,
        omp_dir: bool,
        posh_theme: bool,
    ) -> DetectInputs {
        DetectInputs {
            starship_toml_exists: starship_toml,
            starship_on_path: starship_path,
            zsh_theme: zsh_theme.map(|s| s.to_string()),
            p10k_zsh_exists: p10k,
            oh_my_posh_dir_exists: omp_dir,
            posh_theme_set: posh_theme,
        }
    }

    #[test]
    fn detect_starship_via_toml() {
        let i = inputs(true, false, None, false, false, false);
        assert_eq!(detect_framework(&i), PromptFramework::Starship);
    }

    #[test]
    fn detect_starship_via_path() {
        let i = inputs(false, true, None, false, false, false);
        assert_eq!(detect_framework(&i), PromptFramework::Starship);
    }

    #[test]
    fn detect_p10k_via_zsh_theme() {
        let i = inputs(
            false,
            false,
            Some("powerlevel10k/powerlevel10k"),
            false,
            false,
            false,
        );
        assert_eq!(detect_framework(&i), PromptFramework::Powerlevel10k);
    }

    #[test]
    fn detect_p10k_zsh_theme_case_insensitive() {
        let i = inputs(
            false,
            false,
            Some("Powerlevel10K/Powerlevel10K"),
            false,
            false,
            false,
        );
        assert_eq!(detect_framework(&i), PromptFramework::Powerlevel10k);
    }

    #[test]
    fn detect_p10k_via_p10k_zsh_file() {
        let i = inputs(false, false, None, true, false, false);
        assert_eq!(detect_framework(&i), PromptFramework::Powerlevel10k);
    }

    #[test]
    fn detect_omp_via_dir() {
        let i = inputs(false, false, None, false, true, false);
        assert_eq!(detect_framework(&i), PromptFramework::OhMyPosh);
    }

    #[test]
    fn detect_omp_via_posh_theme() {
        let i = inputs(false, false, None, false, false, true);
        assert_eq!(detect_framework(&i), PromptFramework::OhMyPosh);
    }

    #[test]
    fn detect_generic_fallback() {
        let i = inputs(false, false, None, false, false, false);
        assert_eq!(detect_framework(&i), PromptFramework::Generic);
    }

    #[test]
    fn starship_beats_p10k_when_both_true() {
        let i = inputs(
            true,
            false,
            Some("powerlevel10k/powerlevel10k"),
            false,
            false,
            false,
        );
        assert_eq!(detect_framework(&i), PromptFramework::Starship);
    }

    #[test]
    fn p10k_beats_omp() {
        let i = inputs(false, false, None, true, true, false);
        assert_eq!(detect_framework(&i), PromptFramework::Powerlevel10k);
    }

    #[test]
    fn snippet_starship_has_env_var_module() {
        let s = prompt_snippet(&PromptFramework::Starship);
        assert!(
            s.contains("env_var.thoth_tags"),
            "missing env_var.thoth_tags"
        );
        assert!(s.contains("TTH_PROMPT_TAGS"), "missing TTH_PROMPT_TAGS");
        assert!(
            s.contains("env_var.thoth_tags}"),
            "missing add-to-format note"
        );
        assert!(s.contains(DOCS_ANCHOR), "missing docs anchor");
    }

    #[test]
    fn snippet_p10k_has_key_markers() {
        let s = prompt_snippet(&PromptFramework::Powerlevel10k);
        assert!(s.contains("TTH_PROMPT_TAGS"), "missing TTH_PROMPT_TAGS");
        assert!(s.contains("p10k segment"), "missing p10k segment");
        assert!(s.contains("POWERLEVEL9K"), "missing POWERLEVEL9K");
        assert!(s.contains(DOCS_ANCHOR), "missing docs anchor");
    }

    #[test]
    fn snippet_omp_has_key_markers() {
        let s = prompt_snippet(&PromptFramework::OhMyPosh);
        assert!(s.contains("TTH_PROMPT_TAGS"), "missing TTH_PROMPT_TAGS");
        assert!(
            s.contains(".Env.TTH_PROMPT_TAGS"),
            "missing .Env.TTH_PROMPT_TAGS"
        );
        assert!(s.contains(DOCS_ANCHOR), "missing docs anchor");
    }

    #[test]
    fn snippet_generic_has_key_markers() {
        let s = prompt_snippet(&PromptFramework::Generic);
        let marker = "${TTH_PROMPT_TAGS}";
        assert!(s.contains(marker), "missing ${{TTH_PROMPT_TAGS}}");
        assert!(s.contains("PROMPT"), "missing PROMPT");
        assert!(s.contains("PS1"), "missing PS1");
        assert!(s.contains(DOCS_ANCHOR), "missing docs anchor");
    }

    #[test]
    fn parse_framework_starship() {
        assert_eq!(
            parse_framework("starship").unwrap(),
            PromptFramework::Starship
        );
    }

    #[test]
    fn parse_framework_p10k_aliases() {
        assert_eq!(
            parse_framework("powerlevel10k").unwrap(),
            PromptFramework::Powerlevel10k
        );
        assert_eq!(
            parse_framework("p10k").unwrap(),
            PromptFramework::Powerlevel10k
        );
    }

    #[test]
    fn parse_framework_omp_aliases() {
        assert_eq!(
            parse_framework("oh-my-posh").unwrap(),
            PromptFramework::OhMyPosh
        );
        assert_eq!(
            parse_framework("ohmyposh").unwrap(),
            PromptFramework::OhMyPosh
        );
        assert_eq!(parse_framework("omp").unwrap(), PromptFramework::OhMyPosh);
    }

    #[test]
    fn parse_framework_generic() {
        assert_eq!(
            parse_framework("generic").unwrap(),
            PromptFramework::Generic
        );
    }

    #[test]
    fn parse_framework_unknown_errors() {
        let err = parse_framework("fish").unwrap_err();
        assert!(err.to_string().contains("unknown framework"));
    }
}
