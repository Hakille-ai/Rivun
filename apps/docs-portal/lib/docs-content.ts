import { DocPage, SearchRecord } from './types';
import { GETTING_STARTED_DOCS } from './content/getting-started';
import { ARCHITECTURE_DOCS } from './content/architecture';
import { CONSENSUS_DOCS } from './content/consensus';
import { RUNTIME_DOCS } from './content/runtime';
import { CLOUD_DOCS } from './content/cloud';
import { CRATE_DOCS } from './content/crates';
import { SDK_DOCS } from './content/sdks';
import { DOMAIN_PACK_DOCS } from './content/domain-packs';
import { OPERATIONS_DOCS } from './content/operations';

export const ALL_DOCS: DocPage[] = [
  ...GETTING_STARTED_DOCS,
  ...ARCHITECTURE_DOCS,
  ...CONSENSUS_DOCS,
  ...RUNTIME_DOCS,
  ...CLOUD_DOCS,
  ...CRATE_DOCS,
  ...SDK_DOCS,
  ...DOMAIN_PACK_DOCS,
  ...OPERATIONS_DOCS,
];

const docsByPathMap = new Map<string, DocPage>();
ALL_DOCS.forEach((doc) => {
  docsByPathMap.set(doc.path, doc);
  docsByPathMap.set(doc.slug.join('/'), doc);
});

export function getAllDocs(): DocPage[] {
  return ALL_DOCS;
}

export function getDocBySlug(slug: string[]): DocPage | undefined {
  const key = slug.join('/');
  return docsByPathMap.get(key) || docsByPathMap.get(`/docs/${key}`);
}

export function getAllDocPaths(): string[][] {
  return ALL_DOCS.map((doc) => doc.slug);
}

export function generateSearchIndex(): SearchRecord[] {
  return ALL_DOCS.map((doc) => {
    const headings = doc.headings.map((h) => h.text);
    const keywords: string[] = [
      doc.title,
      doc.section,
      ...(doc.tags || []),
      ...(doc.slug || []),
    ];
    return {
      id: doc.path,
      url: doc.path,
      title: doc.title,
      section: doc.section,
      description: doc.description,
      headings,
      keywords,
      content: `${doc.description} ${doc.rawContent || ''}`.replace(/\s+/g, ' ').trim(),
    };
  });
}
