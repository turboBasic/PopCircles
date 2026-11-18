export interface SettlementCaption {
  name: string;
  country: string;
  distance_km: number;
}

export interface FigureCaption {
  radius_km: number;
  share: number;
  population: number;
  centre_lat: number;
  centre_lon: number;
  settlement: SettlementCaption | null;
  territory_share?: number;
  territory_name?: string;
}

export interface GalleryFigure {
  map: string;
  document: string;
  country_id: string | null;
  alt: string;
  caption?: FigureCaption;
}

export interface GalleryEntry {
  name: string;
  anchor: string;
  figures: GalleryFigure[];
  missing: boolean;
}

export interface GalleryData {
  world: GalleryFigure[];
  countries: GalleryEntry[];
}
