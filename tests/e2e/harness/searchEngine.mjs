export function tokenize(text) {
  if (!text) return [];
  return String(text)
    .toLowerCase()
    .replace(/[^a-z0-9]/g, ' ')
    .split(/\s+/)
    .filter((t) => t.length > 1);
}

export class SearchEngine {
  constructor() {
    this.documents = []; // array of { id, title, category, description, content, keywords, url }
    this.index = new Map(); // token -> array of { docId, freq, field }
  }

  addDocument(doc) {
    const docId = this.documents.length;
    this.documents.push({ ...doc, docId });

    const allKeywords = [...(doc.keywords || []), doc.id, doc.category || ''];

    const fields = [
      { name: 'title', text: doc.title, weight: 5.0 },
      { name: 'keywords', text: allKeywords.join(' '), weight: 3.0 },
      { name: 'description', text: doc.description, weight: 2.0 },
      { name: 'content', text: doc.content, weight: 1.0 },
    ];

    for (const { name, text, weight } of fields) {
      const tokens = tokenize(text);
      const freqs = new Map();
      for (const t of tokens) {
        freqs.set(t, (freqs.get(t) || 0) + 1);
      }
      for (const [t, freq] of freqs.entries()) {
        if (!this.index.has(t)) {
          this.index.set(t, []);
        }
        this.index.get(t).push({ docId, freq, weight, field: name });
      }
    }
  }

  search(query, { category = null, limit = 10 } = {}) {
    const queryTokens = tokenize(query);
    if (queryTokens.length === 0) return [];

    const scores = new Map(); // docId -> score

    for (const qToken of queryTokens) {
      // Direct and prefix matching
      for (const [indexedToken, postings] of this.index.entries()) {
        if (indexedToken === qToken || indexedToken.startsWith(qToken)) {
          const matchMultiplier = indexedToken === qToken ? 1.0 : 0.6;
          for (const post of postings) {
            const currentScore = scores.get(post.docId) || 0;
            const termScore = post.freq * post.weight * matchMultiplier;
            scores.set(post.docId, currentScore + termScore);
          }
        }
      }
    }

    const results = [];
    for (const [docId, score] of scores.entries()) {
      const doc = this.documents[docId];
      if (category && doc.category !== category) continue;

      // Generate snippet with highlighting
      let snippet = doc.description || doc.content.slice(0, 150);
      for (const qToken of queryTokens) {
        const regex = new RegExp('(' + qToken + ')', 'gi');
        snippet = snippet.replace(regex, (match) => '<mark>' + match + '</mark>');
      }

      results.push({
        id: doc.id,
        title: doc.title,
        category: doc.category,
        url: doc.url,
        score,
        snippet,
      });
    }

    results.sort((a, b) => b.score - a.score);
    return results.slice(0, limit);
  }
}
