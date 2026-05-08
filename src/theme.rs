//! Explicit named-role theme primitives.
//!
//! Widgets should style themselves through stable [`ThemeRole`] lookups instead
//! of hardcoding visual policy. [`ThemeConfig`] validates that every required
//! role is present, making missing styles a configuration error rather than an
//! implicit fallback.

use std::collections::BTreeMap;

use ratatui::style::{Color, Modifier, Style};

use crate::config::{ConfigError, Validate};

/// Stable semantic style roles shared by toolkit widgets and applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ThemeRole {
    NormalText,
    DimText,
    Selection,
    SelectionInactive,
    Border,
    FocusedBorder,
    Warning,
    Error,
    Success,
    Accent,
    Background,
    Title,
    Header,
    Footer,
}

impl ThemeRole {
    /// Machine-readable stable role name for diagnostics, config formats, and docs.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NormalText => "normal_text",
            Self::DimText => "dim_text",
            Self::Selection => "selection",
            Self::SelectionInactive => "selection_inactive",
            Self::Border => "border",
            Self::FocusedBorder => "focused_border",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Success => "success",
            Self::Accent => "accent",
            Self::Background => "background",
            Self::Title => "title",
            Self::Header => "header",
            Self::Footer => "footer",
        }
    }
}

/// Required roles for a complete runtime theme.
pub const REQUIRED_THEME_ROLES: [ThemeRole; 14] = [
    ThemeRole::NormalText,
    ThemeRole::DimText,
    ThemeRole::Selection,
    ThemeRole::SelectionInactive,
    ThemeRole::Border,
    ThemeRole::FocusedBorder,
    ThemeRole::Warning,
    ThemeRole::Error,
    ThemeRole::Success,
    ThemeRole::Accent,
    ThemeRole::Background,
    ThemeRole::Title,
    ThemeRole::Header,
    ThemeRole::Footer,
];

/// Explicit map from semantic roles to ratatui styles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeConfig {
    roles: BTreeMap<ThemeRole, Style>,
}

impl ThemeConfig {
    /// Build a theme from an explicit role map, validating that required roles exist.
    pub fn try_explicit(roles: BTreeMap<ThemeRole, Style>) -> Result<Self, ConfigError> {
        let config = Self { roles };
        config.validate()?;
        Ok(config)
    }

    /// A complete high-contrast dark preset suitable for operational TUIs.
    pub fn high_contrast_dark() -> Self {
        let mut roles = BTreeMap::new();
        roles.insert(ThemeRole::NormalText, Style::default().fg(Color::Gray));
        roles.insert(ThemeRole::DimText, Style::default().fg(Color::DarkGray));
        roles.insert(
            ThemeRole::Selection,
            Style::default().fg(Color::Black).bg(Color::Cyan),
        );
        roles.insert(
            ThemeRole::SelectionInactive,
            Style::default().fg(Color::Gray).bg(Color::DarkGray),
        );
        roles.insert(ThemeRole::Border, Style::default().fg(Color::DarkGray));
        roles.insert(ThemeRole::FocusedBorder, Style::default().fg(Color::Cyan));
        roles.insert(ThemeRole::Warning, Style::default().fg(Color::Yellow));
        roles.insert(ThemeRole::Error, Style::default().fg(Color::Red));
        roles.insert(ThemeRole::Success, Style::default().fg(Color::Green));
        roles.insert(ThemeRole::Accent, Style::default().fg(Color::Magenta));
        roles.insert(ThemeRole::Background, Style::default().bg(Color::Black));
        roles.insert(
            ThemeRole::Title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        roles.insert(
            ThemeRole::Header,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        roles.insert(ThemeRole::Footer, Style::default().fg(Color::DarkGray));
        Self { roles }
    }

    /// Return the style for a role. Validation guarantees required roles are present.
    pub fn style(&self, role: ThemeRole) -> Option<Style> {
        self.roles.get(&role).copied()
    }

    /// Override a single role and revalidate the complete theme.
    pub fn try_with_role(mut self, role: ThemeRole, style: Style) -> Result<Self, ConfigError> {
        self.roles.insert(role, style);
        self.validate()?;
        Ok(self)
    }

    /// Iterate over configured roles in deterministic order.
    pub fn roles(&self) -> impl Iterator<Item = (ThemeRole, Style)> + '_ {
        self.roles.iter().map(|(role, style)| (*role, *style))
    }
}

impl Validate for ThemeConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        for role in REQUIRED_THEME_ROLES {
            if !self.roles.contains_key(&role) {
                return Err(ConfigError::new(
                    format!("ThemeConfig.roles.{}", role.as_str()),
                    "missing required theme role; use a named preset such as ThemeConfig::high_contrast_dark() or provide every required role explicitly",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_contains_every_required_role() {
        let theme = ThemeConfig::high_contrast_dark();

        theme.validate().unwrap();
        for role in REQUIRED_THEME_ROLES {
            assert!(theme.style(role).is_some(), "missing {}", role.as_str());
        }
    }

    #[test]
    fn explicit_theme_reports_missing_role_path() {
        let err = ThemeConfig::try_explicit(BTreeMap::new()).unwrap_err();

        assert_eq!(err.path, "ThemeConfig.roles.normal_text");
        assert!(err.reason.contains("missing required theme role"));
    }

    #[test]
    fn role_names_are_stable_and_machine_readable() {
        assert_eq!(ThemeRole::FocusedBorder.as_str(), "focused_border");
        assert_eq!(ThemeRole::SelectionInactive.as_str(), "selection_inactive");
    }
}
