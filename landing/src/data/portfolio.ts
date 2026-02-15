export interface Experience {
  key: string;
  period?: string;
}

export interface Project {
  key: string;
  url: string;
  github?: string;
  techStack: string[];
}

export interface TechItem {
  name: string;
  color: string;
}

export const experiences: Experience[] = [
  { key: 'portfolio.exp.axa', period: '2024' },
  { key: 'portfolio.exp.engie', period: '2023-2024' },
  { key: 'portfolio.exp.sunzu', period: '2022-2023' },
  { key: 'portfolio.exp.denem', period: '2021-2022' },
  { key: 'portfolio.exp.total', period: '2023' },
  { key: 'portfolio.exp.audits', period: '2020-2024' },
];

export const projects: Project[] = [
  {
    key: 'portfolio.proj.ttp',
    url: 'https://github.com/AmirK-S/TTP',
    github: 'https://github.com/AmirK-S/TTP',
    techStack: ['Tauri', 'Rust', 'React', 'Groq API'],
  },
  {
    key: 'portfolio.proj.02viral',
    url: 'https://02viral.com',
    techStack: ['Next.js', 'AI Agents', 'LangChain'],
  },
];

export const techStack: TechItem[] = [
  { name: 'Python', color: '#3776AB' },
  { name: 'Rust', color: '#DEA584' },
  { name: 'TypeScript', color: '#3178C6' },
  { name: 'LangChain', color: '#1C3C3C' },
  { name: 'LangGraph', color: '#1C3C3C' },
  { name: 'Azure', color: '#0078D4' },
  { name: 'MCP', color: '#6366F1' },
  { name: 'RAG', color: '#10B981' },
];
