declare module "react" {
  export = React;
}

declare namespace React {
  export type ReactNode = any;
  export type FC<P = {}> = (props: P) => any;
  export function useState<T>(initial: T | (() => T)): [T, (val: T | ((prev: T) => T)) => void];
  export function useEffect(effect: () => void | (() => void), deps?: any[]): void;
  export function useMemo<T>(factory: () => T, deps: any[]): T;
  export function useCallback<T extends (...args: any[]) => any>(callback: T, deps: any[]): T;
  export function useRef<T>(initialValue?: T): { current: T };
  export const Fragment: any;
  export type FormEvent<T = any> = any;
  export type ChangeEvent<T = any> = any;
  export type MouseEvent<T = any> = any;
}

declare namespace JSX {
  interface IntrinsicElements {
    [elemName: string]: any;
  }
  interface Element extends React.ReactNode {}
}

declare module "next" {
  export interface Metadata {
    title?: string;
    description?: string;
    [key: string]: any;
  }
  export type NextConfig = any;
}

declare module "next/link" {
  const Link: any;
  export default Link;
}

declare module "next/navigation" {
  export function usePathname(): string;
  export function useRouter(): any;
  export function useSearchParams(): any;
}

declare module "lucide-react" {
  export const ShieldCheck: any;
  export const ShieldAlert: any;
  export const Cpu: any;
  export const Terminal: any;
  export const Bell: any;
  export const User: any;
  export const CheckCircle2: any;
  export const ChevronDown: any;
  export const LayoutDashboard: any;
  export const Server: any;
  export const FileCheck: any;
  export const Scale: any;
  export const ShoppingBag: any;
  export const AlertTriangle: any;
  export const Settings: any;
  export const TerminalSquare: any;
  export const Lock: any;
  export const Activity: any;
  export const XCircle: any;
  export const HardDrive: any;
  export const Radio: any;
  export const Clock: any;
  export const Search: any;
  export const SlidersHorizontal: any;
  export const ChevronRight: any;
  export const X: any;
  export const Share2: any;
  export const ArrowRight: any;
  export const ArrowUpRight: any;
  export const Zap: any;
  export const Layers: any;
  export const Key: any;
  export const Copy: any;
  export const Check: any;
  export const Plus: any;
  export const Code: any;
  export const Sliders: any;
  export const Shield: any;
  export const Trash2: any;
  export const Users: any;
  export const Percent: any;
  export const RefreshCw: any;
  export const Download: any;
  export const Building: any;
  export const Bot: any;
  export const Cloud: any;
  export const Factory: any;
  export const HeartPulse: any;
  export const DollarSign: any;
  export const Filter: any;
  export const ExternalLink: any;
  export const Hash: any;
}

declare module "recharts" {
  export const ResponsiveContainer: any;
  export const AreaChart: any;
  export const Area: any;
  export const BarChart: any;
  export const Bar: any;
  export const LineChart: any;
  export const Line: any;
  export const XAxis: any;
  export const YAxis: any;
  export const Tooltip: any;
}

declare module "tailwindcss" {
  export type Config = any;
}

declare var process: {
  env: {
    [key: string]: string | undefined;
  };
};
