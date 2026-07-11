import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { BookOpen, Box } from 'lucide-react';
import { site } from './site';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: site.name,
      url: site.docsRoute,
    },
    githubUrl: site.repository,
    links: [
      {
        type: 'main',
        text: 'crates.io',
        url: site.crates,
        external: true,
        icon: <Box aria-hidden="true" />,
      },
      {
        type: 'main',
        text: 'API docs',
        url: site.rustdoc,
        external: true,
        icon: <BookOpen aria-hidden="true" />,
      },
    ],
    themeSwitch: { enabled: true },
    searchToggle: { enabled: true },
  };
}
