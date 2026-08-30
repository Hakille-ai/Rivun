export interface NavItem {
  title: string;
  href: string;
  badge?: string;
  isNew?: boolean;
  icon?: string;
}

export interface NavSection {
  title: string;
  icon?: string;
  items: NavItem[];
}

export interface BreadcrumbItem {
  title: string;
  href: string;
}

export interface HeadingItem {
  id: string;
  text: string;
  level: number;
}

export interface CodeSnippet {
  language: 'rust' | 'typescript' | 'python' | 'go' | 'bash' | 'toml' | 'json' | 'wat';
  title?: string;
  code: string;
}

export interface MultiLangSnippet {
  id: string;
  snippets: Record<string, { title: string; code: string }>;
}

export interface CalloutData {
  type: 'note' | 'tip' | 'important' | 'warning' | 'security' | 'invariant';
  title?: string;
  content: string;
}

export interface DocPage {
  slug: string[]; // e.g. ["getting-started", "overview"]
  path: string; // e.g. "/docs/getting-started/overview"
  title: string;
  description: string;
  section: string;
  subSection?: string;
  headings: HeadingItem[];
  contentHtml?: string;
  rawContent?: string;
  lastUpdated?: string;
  prev?: { title: string; href: string };
  next?: { title: string; href: string };
  tags?: string[];
  callouts?: CalloutData[];
  multiLangSnippets?: MultiLangSnippet[];
}

export interface SearchRecord {
  id: string;
  url: string;
  title: string;
  section: string;
  description: string;
  headings: string[];
  keywords: string[];
  content: string;
}

export interface ApiEndpoint {
  id: string;
  method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  path: string;
  title: string;
  description: string;
  tags: string[];
  authRequired: boolean;
  headers?: Record<string, string>;
  queryParams?: Array<{ name: string; type: string; required: boolean; description: string; default?: string }>;
  requestBody?: {
    contentType: string;
    schemaExample: string;
  };
  responses: Array<{
    status: number;
    description: string;
    example: string;
  }>;
}
