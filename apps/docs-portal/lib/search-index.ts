import { SearchRecord } from './types';

export interface SearchResult {
  record: SearchRecord;
  score: number;
  highlightTitle: string;
  matchedSnippet: string;
  matchedField: 'title' | 'heading' | 'keyword' | 'content';
}

export class SearchEngine {
  private records: SearchRecord[] = [];
  private isLoaded = false;

  constructor(initialRecords: SearchRecord[] = []) {
    this.records = initialRecords;
    if (initialRecords.length > 0) {
      this.isLoaded = true;
    }
  }

  public setRecords(records: SearchRecord[]) {
    this.records = records;
    this.isLoaded = true;
  }

  public async loadFromPublic(): Promise<void> {
    if (this.isLoaded && this.records.length > 0) return;
    try {
      const res = await fetch('/search-index.json');
      if (res.ok) {
        this.records = await res.json();
        this.isLoaded = true;
      }
    } catch (e) {
      console.warn('Could not load /search-index.json:', e);
    }
  }

  public search(query: string, categoryFilter: string = 'All'): SearchResult[] {
    const rawQuery = query.trim();
    if (!rawQuery) return [];

    const lowerQuery = rawQuery.toLowerCase();
    const queryTokens = lowerQuery.split(/\s+/).filter(Boolean);

    const results: SearchResult[] = [];

    for (const record of this.records) {
      // Category filter check
      if (categoryFilter !== 'All') {
        const matchesCategory = this.checkCategoryMatch(record.section, categoryFilter);
        if (!matchesCategory) continue;
      }

      let score = 0;
      let matchedField: 'title' | 'heading' | 'keyword' | 'content' = 'content';
      let matchedSnippet = record.description;

      const lowerTitle = record.title.toLowerCase();
      const lowerDescription = record.description.toLowerCase();
      const lowerContent = record.content.toLowerCase();
      const lowerKeywords = record.keywords.map((k) => k.toLowerCase()).join(' ');
      const lowerHeadings = record.headings.map((h) => h.toLowerCase()).join(' ');

      // Exact title match (highest score)
      if (lowerTitle === lowerQuery) {
        score += 100;
        matchedField = 'title';
      } else if (lowerTitle.includes(lowerQuery)) {
        score += 50;
        matchedField = 'title';
      }

      // Token matching on title
      for (const token of queryTokens) {
        if (lowerTitle.includes(token)) {
          score += 25;
          matchedField = 'title';
        }
      }

      // Keyword matching
      for (const token of queryTokens) {
        if (lowerKeywords.includes(token)) {
          score += 20;
          if (matchedField !== 'title') matchedField = 'keyword';
        }
      }

      // Heading matching
      for (const h of record.headings) {
        if (h.toLowerCase().includes(lowerQuery)) {
          score += 30;
          matchedField = 'heading';
          matchedSnippet = `Section: ${h}`;
          break;
        }
      }

      // Description matching
      if (lowerDescription.includes(lowerQuery)) {
        score += 15;
      }

      // Content matching
      for (const token of queryTokens) {
        if (lowerContent.includes(token)) {
          score += 5;
          if (matchedField === 'content') {
            matchedSnippet = this.extractSnippet(record.content, token);
          }
        }
      }

      if (score > 0) {
        results.push({
          record,
          score,
          highlightTitle: this.highlightMatches(record.title, queryTokens),
          matchedSnippet: this.highlightMatches(matchedSnippet, queryTokens),
          matchedField,
        });
      }
    }

    return results.sort((a, b) => b.score - a.score).slice(0, 15);
  }

  private checkCategoryMatch(section: string, category: string): boolean {
    const sec = section.toLowerCase();
    switch (category.toLowerCase()) {
      case 'getting started':
        return sec.includes('getting started');
      case 'protocol':
      case 'architecture':
        return sec.includes('architecture') || sec.includes('consensus');
      case 'crates':
        return sec.includes('crate');
      case 'sdks':
        return sec.includes('sdk');
      case 'packs':
      case 'domain packs':
        return sec.includes('domain pack') || sec.includes('store');
      case 'cloud':
        return sec.includes('cloud') || sec.includes('operator');
      case 'operations':
      case 'forensics':
        return sec.includes('fleet') || sec.includes('forensics');
      case 'tools':
      case 'sandboxes':
        return sec.includes('interactive') || sec.includes('sandbox');
      default:
        return true;
    }
  }

  private extractSnippet(content: string, term: string, radius: number = 60): string {
    const idx = content.toLowerCase().indexOf(term.toLowerCase());
    if (idx === -1) return content.slice(0, 120) + '...';
    const start = Math.max(0, idx - radius);
    const end = Math.min(content.length, idx + term.length + radius);
    return (start > 0 ? '...' : '') + content.slice(start, end).trim() + (end < content.length ? '...' : '');
  }

  private highlightMatches(text: string, tokens: string[]): string {
    // For safe string rendering, return clean snippet
    return text;
  }
}

export const globalSearchEngine = new SearchEngine();
