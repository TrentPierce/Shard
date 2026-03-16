import { getAuthor } from "@/lib/authors"
import Image from "next/image"
import Link from "next/link"
import { notFound } from "next/navigation"
import { Github, Linkedin, GraduationCap, Twitter } from "lucide-react"
import { Metadata } from "next"

interface AuthorPageProps {
  params: {
    slug: string
  }
}

export async function generateMetadata({ params }: AuthorPageProps): Promise<Metadata> {
  const author = getAuthor(params.slug)
  if (!author) return {}

  return {
    title: `${author.name} | Shard Author`,
    description: author.bio,
  }
}

export default function AuthorPage({ params }: AuthorPageProps) {
  const author = getAuthor(params.slug)

  if (!author) {
    notFound()
  }

  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "Person",
    name: author.name,
    jobTitle: author.role,
    description: author.bio,
    image: author.photo,
    url: `https://shardnetwork.live/authors/${author.slug}`,
    sameAs: [
      author.linkedin,
      author.github,
      author.twitter,
      author.googleScholar,
    ].filter(Boolean),
  }

  return (
    <main className="mx-auto max-w-4xl px-4 py-20 sm:px-6 lg:px-8">
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
      />
      
      <div className="flex flex-col items-center gap-8 md:flex-row md:items-start">
        <div className="relative h-48 w-48 overflow-hidden rounded-2xl border border-white/10 bg-white/5">
          <Image
            src={author.photo}
            alt={author.name}
            fill
            className="object-cover"
            priority
          />
        </div>

        <div className="flex-1 space-y-6 text-center md:text-left">
          <div>
            <h1 className="text-3xl font-bold tracking-tight text-ink-100 sm:text-4xl">
              {author.name}
            </h1>
            <p className="mt-2 text-lg font-medium text-accent-300">{author.role}</p>
          </div>

          <p className="text-lg leading-relaxed text-ink-300">{author.bio}</p>

          <div className="flex flex-wrap justify-center gap-4 md:justify-start">
            {author.github && (
              <a
                href={author.github}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm font-medium text-ink-200 transition-colors hover:bg-white/10 hover:text-ink-100"
              >
                <Github size={18} />
                GitHub
              </a>
            )}
            {author.linkedin && (
              <a
                href={author.linkedin}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm font-medium text-ink-200 transition-colors hover:bg-white/10 hover:text-ink-100"
              >
                <Linkedin size={18} />
                LinkedIn
              </a>
            )}
            {author.googleScholar && (
              <a
                href={author.googleScholar}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm font-medium text-ink-200 transition-colors hover:bg-white/10 hover:text-ink-100"
              >
                <GraduationCap size={18} />
                Scholar
              </a>
            )}
            {author.twitter && (
              <a
                href={author.twitter}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm font-medium text-ink-200 transition-colors hover:bg-white/10 hover:text-ink-100"
              >
                <Twitter size={18} />
                Twitter
              </a>
            )}
          </div>
        </div>
      </div>
    </main>
  )
}
