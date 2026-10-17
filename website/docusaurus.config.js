// @ts-check
import {themes as prismThemes} from 'prism-react-renderer';

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'gpui-proptest',
  tagline: 'Property-based testing and fuzzing for GPUI',
  favicon: 'img/logo.svg',

  url: 'https://ziinc.github.io',
  baseUrl: '/gpui-proptest/',
  organizationName: 'Ziinc',
  projectName: 'gpui-proptest',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  markdown: {hooks: {onBrokenMarkdownLinks: 'throw'}},

  i18n: {defaultLocale: 'en', locales: ['en']},

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          routeBasePath: '/',
          sidebarPath: './sidebars.js',
          editUrl: 'https://github.com/Ziinc/gpui-proptest/tree/main/website/',
        },
        blog: false,
        theme: {customCss: './src/css/custom.css'},
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      colorMode: {respectPrefersColorScheme: true},
      navbar: {
        title: 'gpui-proptest',
        logo: {alt: 'gpui-proptest', src: 'img/logo.svg'},
        items: [
          {type: 'docSidebar', sidebarId: 'docs', position: 'left', label: 'Docs'},
          {to: '/engines/proptest', label: 'Engines', position: 'left'},
          {to: '/best-practices', label: 'Best practices', position: 'left'},
          {
            href: 'https://github.com/Ziinc/gpui-proptest',
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
              {label: 'Getting started', to: '/getting-started'},
              {label: 'proptest engine', to: '/engines/proptest'},
              {label: 'bolero engine', to: '/engines/bolero'},
            ],
          },
          {
            title: 'Related',
            items: [
              {label: 'GPUI', href: 'https://www.gpui.rs'},
              {label: 'GPUI Kit', href: 'https://gpui-kit.com'},
              {label: 'proptest', href: 'https://proptest-rs.github.io/proptest/'},
              {label: 'bolero', href: 'https://camshaft.github.io/bolero/'},
            ],
          },
        ],
        copyright: `MIT licensed. Built with Docusaurus.`,
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
        additionalLanguages: ['rust', 'toml', 'bash'],
      },
    }),
};

export default config;
