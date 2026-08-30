import { NextResponse } from 'next/server';
import { generateSearchIndex } from '@/lib/docs-content';

export async function GET() {
  const records = generateSearchIndex();
  return NextResponse.json(records, {
    headers: {
      'Cache-Control': 'public, max-age=3600, s-maxage=3600, stale-while-revalidate=86400',
    },
  });
}
