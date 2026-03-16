export interface Author {
  slug: string
  name: string
  role: string
  bio: string
  photo: string
  linkedin?: string
  googleScholar?: string
  twitter?: string
  github?: string
}

export const authors: Author[] = [
  {
    slug: "trent-pierce",
    name: "Trent Pierce",
    role: "Lead Architect & Founder",
    bio: "Trent Pierce is a software engineer focused on distributed systems and AI observability. He is the primary creator of Shard Network, a receipt-first workflow runtime designed to bring transparency to agentic workflows. With a background in high-performance computing and developer tooling, Trent builds systems that prioritize empirical evidence and verifiable execution.",
    photo: "https://github.com/TrentPierce.png",
    linkedin: "https://linkedin.com/in/trentpierce", // Placeholder
    github: "https://github.com/TrentPierce",
  },
]

export function getAuthor(slug: string): Author | undefined {
  return authors.find((a) => a.slug === slug)
}
