import { defineConfig } from 'vocs'
import { sidebar } from './sidebar'

export default defineConfig({
  title: 'Kona',
  description: 'Modular, performant, and secure OP Stack infrastructure in Rust',
  logoUrl: '/logo.png',
  iconUrl: '/logo.png',
  ogImageUrl: '/kona-prod.png',
  sidebar,
  head: [
    ['script', {}, `
      document.addEventListener('DOMContentLoaded', function() {
        const footer = document.createElement('div');
        footer.className = 'vocs-custom-footer';
        footer.innerHTML = 'Built by <a href="https://oplabs.co" target="_blank" rel="noopener noreferrer">OP Labs</a> and open source contributors. • <a href="https://github.com/op-rs/kona" target="_blank" rel="noopener noreferrer">GitHub</a>';
        document.body.appendChild(footer);
      });
    `]
  ],
  topNav: [
    { text: 'Run', link: '/node/run/overview' },
    { text: 'SDK', link: '/sdk/overview' },
    { text: 'Rustdocs', link: 'https://docs.rs/kona-node/latest/' },
    { text: 'GitHub', link: 'https://github.com/op-rs/kona' },
    {
      text: 'v0.1.0',
      items: [
        {
          text: 'Releases',
          link: 'https://github.com/op-rs/kona/releases'
        },
        {
          text: 'Contributing',
          link: 'https://github.com/op-rs/kona/blob/main/CONTRIBUTING.md'
        }
      ]
    }
  ],
  socials: [
    {
      icon: 'github',
      link: 'https://github.com/op-rs/kona',
    },
  ],
  theme: {
    accentColor: {
      light: '#1f1f1f',
      dark: '#ffffff'
    }
  },
  sponsors: [
    {
      name: 'Supporters',
      height: 120,
      items: [
        [
          {
            name: 'OP Labs',
            link: 'https://oplabs.co',
            image: 'https://avatars.githubusercontent.com/u/109625874?s=200&v=4',
          }
        ]
      ]
    }
  ]
})
