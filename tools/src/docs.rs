//! What the site mirrors from the kernel repository, and where each page lands.

/// One mirrored document.
pub struct Doc {
    /// Path in the kernel repository, from its root.
    pub source: &'static str,
    /// The site path, under `/learn`.
    pub slug: &'static str,
    /// Title, as it appears on the page and in the index.
    pub title: &'static str,
    /// Which part of the index it belongs to.
    pub section: Section,
}

/// A group in the documentation index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Start,
    Tutorial,
    Plugins,
    Design,
    Project,
}

impl Section {
    pub fn title(self) -> &'static str {
        match self {
            Self::Start => "Start here",
            Self::Tutorial => "The tutorial",
            Self::Plugins => "Writing plugins",
            Self::Design => "How it works",
            Self::Project => "The project",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Start => "Getting a site running and putting content in it.",
            Self::Tutorial => "Nine parts, building one real site from an empty database.",
            Self::Plugins => "The plugin contract, the taps, and what a plugin may reach.",
            Self::Design => {
                "The design documents: what each part does and why it is shaped that way."
            }
            Self::Project => "Standards, known problems, and how the work is organised.",
        }
    }

    /// Index order.
    pub fn weight(self) -> u32 {
        match self {
            Self::Start => 0,
            Self::Tutorial => 100,
            Self::Plugins => 200,
            Self::Design => 300,
            Self::Project => 400,
        }
    }
}

/// The corpus.
///
/// Named one by one rather than globbed. A glob would mirror whatever happened
/// to be in `docs/` at the pinned tag, including working notes and audits
/// written for the person building the kernel rather than for someone reading
/// about it, and it would silently change what the site publishes every time the
/// kernel adds a file. This list is a decision about what the site is for.
pub const DOCS: &[Doc] = &[
    // ── Start here ──
    Doc {
        source: "README.md",
        slug: "readme",
        title: "Read me first",
        section: Section::Start,
    },
    Doc {
        source: "INSTALL.md",
        slug: "install",
        title: "Installing Trovato",
        section: Section::Start,
    },
    Doc {
        source: "docs/building-your-first-site.md",
        slug: "building-your-first-site",
        title: "Building your first site",
        section: Section::Start,
    },
    Doc {
        source: "docs/docker-development.md",
        slug: "docker-development",
        title: "Developing with Docker",
        section: Section::Start,
    },
    // ── The tutorial ──
    Doc {
        source: "docs/tutorial/part-01-hello-trovato.md",
        slug: "tutorial-01-hello-trovato",
        title: "Part 1: Hello, Trovato",
        section: Section::Tutorial,
    },
    Doc {
        source: "docs/tutorial/part-02-ritrovo-importer.md",
        slug: "tutorial-02-importer",
        title: "Part 2: The importer",
        section: Section::Tutorial,
    },
    Doc {
        source: "docs/tutorial/part-03-look-and-feel.md",
        slug: "tutorial-03-look-and-feel",
        title: "Part 3: Look and feel",
        section: Section::Tutorial,
    },
    Doc {
        source: "docs/tutorial/part-04-editorial-engine.md",
        slug: "tutorial-04-editorial-engine",
        title: "Part 4: The editorial engine",
        section: Section::Tutorial,
    },
    Doc {
        source: "docs/tutorial/part-05-forms-and-input.md",
        slug: "tutorial-05-forms-and-input",
        title: "Part 5: Forms and input",
        section: Section::Tutorial,
    },
    Doc {
        source: "docs/tutorial/part-06-community.md",
        slug: "tutorial-06-community",
        title: "Part 6: Community",
        section: Section::Tutorial,
    },
    Doc {
        source: "docs/tutorial/part-07-going-global.md",
        slug: "tutorial-07-going-global",
        title: "Part 7: Going global",
        section: Section::Tutorial,
    },
    Doc {
        source: "docs/tutorial/part-08-production-ready.md",
        slug: "tutorial-08-production-ready",
        title: "Part 8: Production ready",
        section: Section::Tutorial,
    },
    Doc {
        source: "docs/tutorial/part-09-ai-and-search.md",
        slug: "tutorial-09-ai-and-search",
        title: "Part 9: AI and search",
        section: Section::Tutorial,
    },
    // ── Writing plugins ──
    Doc {
        source: "docs/plugin-development.md",
        slug: "plugin-development",
        title: "The plugin development guide",
        section: Section::Plugins,
    },
    Doc {
        source: "docs/plugin-quick-reference.md",
        slug: "plugin-quick-reference",
        title: "Plugin quick reference",
        section: Section::Plugins,
    },
    Doc {
        source: "docs/plugin-error-codes.md",
        slug: "plugin-error-codes",
        title: "Plugin error codes",
        section: Section::Plugins,
    },
    Doc {
        source: "docs/plugin-queue.md",
        slug: "plugin-queue",
        title: "The plugin queue",
        section: Section::Plugins,
    },
    Doc {
        source: "docs/api-reference.md",
        slug: "api-reference",
        title: "API reference",
        section: Section::Plugins,
    },
    // ── How it works ──
    Doc {
        source: "docs/design/Overview.md",
        slug: "design-overview",
        title: "Overview",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Architecture.md",
        slug: "design-architecture",
        title: "Architecture",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Terminology.md",
        slug: "design-terminology",
        title: "Terminology: the Drupal words and the Trovato ones",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Design-Content-Model.md",
        slug: "design-content-model",
        title: "The content model",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Design-Query-Engine.md",
        slug: "design-query-engine",
        title: "Gather, the query engine",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Design-Plugin-System.md",
        slug: "design-plugin-system",
        title: "The plugin system",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Design-Plugin-SDK.md",
        slug: "design-plugin-sdk",
        title: "The plugin SDK",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Design-Render-Theme.md",
        slug: "design-render-theme",
        title: "Rendering and theming",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Design-Web-Layer.md",
        slug: "design-web-layer",
        title: "The web layer",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Design-Infrastructure.md",
        slug: "design-infrastructure",
        title: "Infrastructure",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/search-architecture.md",
        slug: "design-search",
        title: "Search architecture",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/ai-integration.md",
        slug: "design-ai",
        title: "AI integration",
        section: Section::Design,
    },
    Doc {
        source: "docs/design/Versioning.md",
        slug: "design-versioning",
        title: "Versioning",
        section: Section::Design,
    },
    // ── The project ──
    Doc {
        source: "docs/coding-standards.md",
        slug: "coding-standards",
        title: "Coding standards",
        section: Section::Project,
    },
    Doc {
        source: "ROADMAP.md",
        slug: "roadmap",
        title: "Roadmap",
        section: Section::Project,
    },
    Doc {
        source: "CONTRIBUTING.md",
        slug: "contributing",
        title: "Contributing",
        section: Section::Project,
    },
    Doc {
        source: "CODE_OF_CONDUCT.md",
        slug: "code-of-conduct",
        title: "Code of conduct",
        section: Section::Project,
    },
];

/// The known-issues page, which the site treats differently from the rest: it is
/// dated on the page, because a list of what is broken is worth nothing without
/// the date it was true.
pub const KNOWN_ISSUES: Doc = Doc {
    source: "KNOWN-ISSUES.md",
    slug: "known-issues",
    title: "Known issues",
    section: Section::Project,
};

/// Every document, in index order.
pub fn all() -> Vec<&'static Doc> {
    let mut docs: Vec<&'static Doc> = DOCS.iter().collect();
    docs.push(&KNOWN_ISSUES);
    docs.sort_by_key(|d| {
        (
            d.section.weight(),
            DOCS.iter()
                .position(|x| x.slug == d.slug)
                .unwrap_or(usize::MAX),
        )
    });
    docs
}
