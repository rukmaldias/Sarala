// @ts-check
// Docs live in the repo alongside the code they describe (../docs and
// ../boards); this site is a pure rendering layer over them — no docs move.
import {themes as prismThemes} from 'prism-react-renderer';

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'Sarala',
  tagline: 'A minimal, non-Android mobile operating system for the Google Pixel XL',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  // GitHub Pages target (https://rukmaldias.github.io/Sarala/).
  url: 'https://rukmaldias.github.io',
  baseUrl: '/Sarala/',
  organizationName: 'rukmaldias',
  projectName: 'Sarala',

  // Existing docs cross-link with repo-relative paths that don't all resolve
  // under Docusaurus routing yet — warn rather than fail the build for now.
  onBrokenLinks: 'warn',

  // Parse .md as CommonMark (not MDX): the existing docs use <https://…>
  // autolinks and other angle brackets that MDX misreads as JSX.
  markdown: {
    format: 'detect',
    hooks: {
      onBrokenMarkdownLinks: 'warn',
    },
  },

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          path: '../docs',
          routeBasePath: 'docs',
          sidebarPath: './sidebars.js',
          editUrl: 'https://github.com/rukmaldias/Sarala/edit/master/docs/',
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      }),
    ],
  ],

  // Second docs instance: the board / hardware notes under ../boards.
  plugins: [
    [
      '@docusaurus/plugin-content-docs',
      /** @type {import('@docusaurus/plugin-content-docs').Options} */
      ({
        id: 'boards',
        path: '../boards',
        routeBasePath: 'boards',
        sidebarPath: './sidebarsBoards.js',
        editUrl: 'https://github.com/rukmaldias/Sarala/edit/master/boards/',
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      image: 'img/docusaurus-social-card.jpg',
      colorMode: {
        respectPrefersColorScheme: true,
      },
      navbar: {
        title: 'Sarala',
        items: [
          {
            type: 'docSidebar',
            sidebarId: 'docs',
            position: 'left',
            label: 'Design & Roadmap',
          },
          {
            type: 'docSidebar',
            docsPluginId: 'boards',
            sidebarId: 'boards',
            position: 'left',
            label: 'Boards & Hardware',
          },
          {
            href: 'https://github.com/rukmaldias/Sarala',
            label: 'GitHub',
            position: 'right',
          },
        ],
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Docs',
            items: [
              {label: 'Roadmap', to: '/docs/roadmap'},
              {label: 'Architecture', to: '/docs/architecture'},
              {label: 'Marlin bring-up', to: '/boards/marlin/'},
            ],
          },
          {
            title: 'Project',
            items: [
              {label: 'GitHub', href: 'https://github.com/rukmaldias/Sarala'},
            ],
          },
        ],
        copyright: `Sarala — built ${new Date().getFullYear()}. Rendered with Docusaurus.`,
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
        additionalLanguages: ['bash', 'toml', 'rust'],
      },
    }),
};

export default config;
