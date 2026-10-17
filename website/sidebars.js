// @ts-check

/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  docs: [
    'intro',
    'getting-started',
    {
      type: 'category',
      label: 'Concepts',
      collapsed: false,
      items: [
        'concepts/targets',
        'concepts/steps',
        'concepts/oracles-and-invariants',
        'concepts/determinism-and-replay',
      ],
    },
    {
      type: 'category',
      label: 'Engines',
      collapsed: false,
      items: ['engines/overview', 'engines/proptest', 'engines/bolero'],
    },
    {
      type: 'category',
      label: 'Guides',
      collapsed: false,
      items: [
        'guides/model-based-testing',
        'guides/gpui-kit',
        'guides/custom-windows-and-targets',
        'guides/reading-a-report',
      ],
    },
    'best-practices',
    {
      type: 'category',
      label: 'Reference',
      items: ['reference/configuration', 'reference/troubleshooting', 'reference/limitations'],
    },
  ],
};

export default sidebars;
