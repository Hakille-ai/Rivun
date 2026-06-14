import DocsSidebar from "@/components/layout/DocsSidebar";

export const metadata = {
  title: 'Documentation | ZAP Protocol',
  description: 'Technical documentation for the ZAP protocol.',
};

export default function DocsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="flex-1 min-h-0 flex gap-12 max-w-7xl mx-auto w-full px-6 overflow-hidden">
      <DocsSidebar />
      <div className="flex-1 overflow-y-auto py-12 min-w-0 h-full scroll-smooth">
        <div className="prose prose-invert prose-blue max-w-4xl">
          {children}
        </div>
      </div>
    </div>
  );
}
