//! Allocation-conscious comma-separated protocol-header lists.
//!
//! The request context owns parsed header values as `Box<str>` and consumes
//! this private iterator without eagerly allocating owned tokens.

#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct HeaderList {
    raw: Option<Box<str>>,
}

impl HeaderList {
    pub(crate) fn parse(value: Option<&str>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };

        if value.split(',').map(str::trim).all(str::is_empty) {
            return Self::default();
        }

        Self {
            raw: Some(value.into()),
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &str> + '_ {
        self.raw
            .as_deref()
            .into_iter()
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub(crate) fn contains(&self, expected: &str) -> bool {
        self.iter().any(|value| value == expected)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }
}

impl std::fmt::Debug for HeaderList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_list().entries(self.iter()).finish()
    }
}
