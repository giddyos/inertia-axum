import { createMDX } from 'fumadocs-mdx/next';

const withMDX = createMDX();

/** @type {import('next').NextConfig} */
const config = {
  reactStrictMode: true,
  poweredByHeader: false,
  turbopack: {
    root: import.meta.dirname,
  },
  async rewrites() {
    return [
      {
        source: '/docs.md',
        destination: '/llms.mdx/docs/content.md',
      },
      {
        source: '/docs/:path*.md',
        destination: '/llms.mdx/docs/:path*/content.md',
      },
    ];
  },
};

export default withMDX(config);
