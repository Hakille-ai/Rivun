import React from 'react';
import { notFound } from 'next/navigation';
import Link from 'next/link';
import { ArrowLeft, ArrowRight, Clock, Tag } from 'lucide-react';
import { getAllDocPaths, getDocBySlug } from '@/lib/docs-content';
import { findPrevNextNav } from '@/lib/navigation';
import { Breadcrumbs } from '@/components/layout/Breadcrumbs';
import { TableOfContents } from '@/components/layout/TableOfContents';
import { CodeTabs } from '@/components/ui/CodeTabs';
import { Callout } from '@/components/ui/Callout';
import { Badge } from '@/components/ui/Badge';
import { MathFormula } from '@/components/ui/MathFormula';
import { Mermaid } from '@/components/ui/Mermaid';

interface DocPageProps {
  params: Promise<{
    slug: string[];
  }>;
}

export async function generateStaticParams() {
  const paths = getAllDocPaths();
  return paths.map((slug) => ({ slug }));
}

export default async function DocSlugPage({ params }: DocPageProps) {
  const { slug } = await params;
  const doc = getDocBySlug(slug);

  if (!doc) {
    notFound();
  }

  const { prev, next } = findPrevNextNav(doc.path);

  // Build Breadcrumbs list
  const breadcrumbItems = [
    { title: doc.section, href: doc.path },
    { title: doc.title, href: doc.path },
  ];

  return (
    <div className="flex flex-col lg:flex-row gap-8">
      {/* Main Center Article */}
      <article className="flex-1 min-w-0 docs-prose">
        {/* Breadcrumbs */}
        <Breadcrumbs items={breadcrumbItems} />

        {/* Section Badge & Meta */}
        <div className="flex items-center gap-2 mb-3">
          <Badge variant="cyan">{doc.section}</Badge>
          {doc.subSection && <Badge variant="outline">{doc.subSection}</Badge>}
        </div>

        {/* Page Title */}
        <h1 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-text-primary mb-3">
          {doc.title}
        </h1>

        {/* Page Description */}
        <p className="text-base text-text-secondary leading-relaxed mb-6 border-b border-border-subtle pb-6">
          {doc.description}
        </p>

        {/* Callouts */}
        {doc.callouts && doc.callouts.length > 0 && (
          <div className="my-6 space-y-4">
            {doc.callouts.map((callout, idx) => (
              <Callout key={idx} type={callout.type} title={callout.title}>
                <p>{callout.content}</p>
              </Callout>
            ))}
          </div>
        )}

        {/* Multi-language code tabs if present */}
        {doc.multiLangSnippets && doc.multiLangSnippets.length > 0 && (
          <div className="my-6 space-y-6">
            {doc.multiLangSnippets.map((snippet) => (
              <CodeTabs key={snippet.id} tabs={snippet.snippets} />
            ))}
          </div>
        )}

        {/* Prose Body Content */}
        {doc.rawContent && (
          <div className="space-y-4 leading-relaxed text-text-secondary text-sm">
            {doc.rawContent.split('\n\n').map((paragraph, idx) => {
              const trimmed = paragraph.trim();
              if (!trimmed) return null;

              if (trimmed.startsWith('### ')) {
                const headingText = trimmed.replace('### ', '');
                const headingId = headingText
                  .toLowerCase()
                  .replace(/[^a-z0-9]+/g, '-')
                  .replace(/(^-|-$)/g, '');
                return (
                  <h3 key={idx} id={headingId} className="text-lg font-bold text-text-primary mt-8 mb-2">
                    {headingText}
                  </h3>
                );
              }

              if (trimmed.startsWith('## ')) {
                const headingText = trimmed.replace('## ', '');
                const headingId = headingText
                  .toLowerCase()
                  .replace(/[^a-z0-9]+/g, '-')
                  .replace(/(^-|-$)/g, '');
                return (
                  <h2 key={idx} id={headingId} className="text-xl font-bold text-text-primary mt-10 mb-3 border-b border-border-subtle pb-2">
                    {headingText}
                  </h2>
                );
              }

              if (trimmed.startsWith('```text') || trimmed.startsWith('```mermaid')) {
                const clean = trimmed.replace(/^```(text|mermaid)\n/, '').replace(/\n```$/, '');
                return <Mermaid key={idx} chart={clean} title="Protocol Architecture Diagram" />;
              }

              if (trimmed.startsWith('$$\\text{') || trimmed.startsWith('$$')) {
                const clean = trimmed.replace(/^\$\$/, '').replace(/\$\$$/, '').trim();
                return <MathFormula key={idx} formula={clean} />;
              }

              return (
                <p key={idx} className="leading-relaxed">
                  {trimmed}
                </p>
              );
            })}
          </div>
        )}

        {/* Bottom Pagination: Previous / Next page links */}
        <div className="mt-14 pt-6 border-t border-border-subtle flex flex-col sm:flex-row items-center justify-between gap-4 not-prose">
          {prev ? (
            <Link
              href={prev.href}
              className="flex items-center gap-2 p-3 rounded-xl border border-border-subtle bg-bg-surface hover:bg-bg-surface-raised text-text-secondary hover:text-text-primary transition-all text-xs w-full sm:w-auto hover:border-accent-primary/40"
            >
              <ArrowLeft className="w-4 h-4 text-accent-primary shrink-0" />
              <div className="text-left truncate">
                <div className="text-[10px] text-text-muted">Previous</div>
                <div className="font-semibold text-text-primary truncate">{prev.title}</div>
              </div>
            </Link>
          ) : (
            <div />
          )}

          {next ? (
            <Link
              href={next.href}
              className="flex items-center justify-end gap-2 p-3 rounded-xl border border-border-subtle bg-bg-surface hover:bg-bg-surface-raised text-text-secondary hover:text-text-primary transition-all text-xs w-full sm:w-auto text-right hover:border-accent-primary/40"
            >
              <div className="text-right truncate">
                <div className="text-[10px] text-text-muted">Next</div>
                <div className="font-semibold text-text-primary truncate">{next.title}</div>
              </div>
              <ArrowRight className="w-4 h-4 text-accent-primary shrink-0" />
            </Link>
          ) : (
            <div />
          )}
        </div>
      </article>

      {/* Right Table of Contents (Sticky on Desktop) */}
      <aside className="hidden xl:block w-64 shrink-0">
        <div className="sticky top-24 p-4 rounded-xl bg-bg-surface/60 border border-border-subtle backdrop-blur-md">
          <TableOfContents headings={doc.headings} />
        </div>
      </aside>
    </div>
  );
}
