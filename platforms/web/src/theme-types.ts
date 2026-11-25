import type { ThemeName } from "./public-catalog.js";

/** A concrete appearance selected by the host. */
export type ThemeAppearance = "light" | "dark";
export type ThemeCategory = "reading" | "editor" | "art" | "custom";
export type ThemePrecedence = "host" | "source";

export interface ThemeCatalog {
  readonly schema_version: 2;
  readonly presets: readonly ThemePresetCatalogEntry[];
  readonly mermaid_themes: readonly ThemeName[];
  /** Native theme capability bit masks by logical diagram family. */
  readonly families: readonly { id: string; capabilities: number }[];
}

export interface ThemePresetCatalogEntry {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly category: ThemeCategory;
  readonly appearances: readonly ThemeAppearance[];
  readonly default_appearance: ThemeAppearance;
  /** Semantic colors for the preset's default appearance. */
  readonly swatches: readonly [
    canvas: string, node: string, container: string,
    title: string, primaryText: string, firstDataFill: string,
  ];
}

export interface ThemeDocumentV2 {
  readonly $schema?: string;
  readonly schema_version: 2;
  readonly id?: string;
  readonly name?: string;
  readonly description?: string;
  readonly category?: ThemeCategory;
  readonly extends?: string;
  readonly default_appearance?: ThemeAppearance;
  readonly seed?: ThemeSeed;
  readonly roles?: Partial<ThemeRoles>;
  readonly typography?: Partial<ThemeTypography>;
  readonly form?: ThemeFormPatch;
  readonly appearances?: Readonly<Partial<Record<ThemeAppearance, ThemeAppearanceLayer>>>;
  readonly overrides?: readonly ThemeFamilyOverride[];
}

/** A preset ID, `none` to clear the selection, or an editable v2 document. */
export type DiagramThemeSelection =
  | string
  | (ThemeDocumentV2 & {
      readonly preset?: never;
      readonly document?: never;
      readonly appearance?: never;
      readonly precedence?: never;
    })
  | ({
      readonly schema_version?: never;
      readonly appearance?: ThemeAppearance;
      readonly precedence?: ThemePrecedence;
    } & (
      | { readonly preset: string; readonly document?: never }
      | { readonly document: ThemeDocumentV2; readonly preset?: never }
    ));

export type ThemeSeed = Readonly<Partial<Record<
  "canvas" | "ink" | "accent" | "surface" | "callout" | "info" | "success" | "warn" | "danger",
  string
>>>;

export type ThemePaint = string | {
  readonly kind: "linear_gradient";
  readonly angle_deg?: number;
  readonly stops: readonly { offset: number; color: string; opacity?: number }[];
};

export interface ThemeRoles {
  readonly canvas: ThemePaint;
  readonly "surface.node": ThemePaint;
  readonly "surface.container": ThemePaint;
  readonly "surface.callout": ThemePaint;
  readonly "surface.emphasis": ThemePaint;
  readonly "stroke.border": ThemePaint;
  readonly "stroke.connector": ThemePaint;
  readonly "stroke.guide": ThemePaint;
  readonly "text.primary": string;
  readonly "text.muted": string;
  readonly "text.title": string;
  readonly "text.callout": string;
  readonly "text.on_emphasis": string;
  readonly "status.info": string;
  readonly "status.success": string;
  readonly "status.warn": string;
  readonly "status.danger": string;
}

type ThemeTextureColor = { readonly color?: string; readonly opacity?: number };

export type ThemeTexture =
  | { readonly kind: "none" }
  | (ThemeTextureColor & {
      readonly kind: "grid";
      readonly step_px: number;
      readonly major_every?: number;
      readonly major_opacity?: number;
    })
  | (ThemeTextureColor & { readonly kind: "dots"; readonly step_px: number; readonly radius_px?: number })
  | (ThemeTextureColor & {
      readonly kind: "ruled";
      readonly line_height_px: number;
      readonly margin_rule_color?: string;
      readonly margin_rule_x_px?: number;
    })
  | (ThemeTextureColor & { readonly kind: "fibers"; readonly angle_deg?: number; readonly cross?: boolean })
  | (ThemeTextureColor & { readonly kind: "scanlines"; readonly step_px: number })
  | {
      readonly kind: "radial_wash";
      readonly color: string;
      readonly opacity?: number;
      readonly cx?: number;
      readonly cy?: number;
      readonly radius?: number;
    };

interface ThemeShadowLayer {
  readonly dx: number;
  readonly dy: number;
  readonly blur?: number;
  readonly color: string;
  readonly opacity?: number;
}

interface ThemeGlow {
  readonly kind: "glow";
  readonly color: string;
  readonly blur: number;
  readonly opacity?: number;
}

export type ThemeElevationSpec =
  | "flat" | "soft" | "crisp"
  | { readonly kind: "layered_stack"; readonly layers: readonly ThemeShadowLayer[] }
  | ThemeGlow;

export type ThemeElevation =
  | { readonly kind: "flat" }
  | { readonly kind: "shadows"; readonly layers: readonly Required<ThemeShadowLayer>[] }
  | Required<ThemeGlow>;

type ThemeFontRole = "sans" | "serif" | "mono";

export interface ThemeTypography {
  readonly sans: readonly string[];
  readonly mono: readonly string[];
  readonly serif: readonly string[];
  readonly body_family: ThemeFontRole;
  readonly title_family: ThemeFontRole;
  readonly code_family: ThemeFontRole;
  readonly base_size_px: number;
  readonly title_weight: number;
  readonly label_weight: number;
}

export interface ThemeForm {
  readonly density: "compact" | "balanced" | "airy";
  readonly radius: number;
  readonly container_radius: number;
  readonly stroke_width: number;
  readonly connector_width: number;
  readonly edge_style: "curved" | "orthogonal" | "straight" | "sketch";
  readonly marker_style: "stealth" | "filled" | "open" | "classic";
  readonly edge_label_bg: "auto" | "canvas" | "pill" | "transparent";
  readonly chart: {
    readonly gridlines: boolean;
    readonly bar_radius: number;
    readonly bar_wireframe: boolean;
    readonly line_markers: boolean;
    readonly pie_gap_px: number;
    readonly pie_opacity: number;
  };
}

export interface ThemeFormPatch extends Partial<Omit<ThemeForm, "chart">> {
  readonly elevation?: ThemeElevationSpec;
  readonly chart?: Partial<ThemeForm["chart"]>;
}

interface ThemeCanvas {
  readonly textures: readonly ThemeTexture[];
  readonly padding_px: number;
}

interface ThemeDataPalette {
  readonly ink: readonly string[];
  readonly fill: readonly string[];
  readonly fill_ink: readonly string[];
  readonly pattern_encoding: boolean;
}

export interface ThemeAppearanceLayer {
  readonly seed?: ThemeSeed;
  readonly roles?: Partial<ThemeRoles>;
  readonly data?: Partial<Omit<ThemeDataPalette, "fill_ink">> & { readonly fill_ink?: string };
  readonly canvas?: Partial<ThemeCanvas>;
  readonly elevation?: ThemeElevationSpec;
}

export interface ThemeFamilyOverride {
  readonly families: readonly string[];
  readonly appearance?: ThemeAppearance;
  readonly roles?: Partial<ThemeRoles>;
  readonly form?: ThemeFormPatch;
  readonly elevation?: ThemeElevationSpec;
  readonly typography?: Partial<ThemeTypography>;
  readonly node_ordinal_palette?: boolean;
}

interface ResolvedFamilyTheme {
  readonly roles: ThemeRoles;
  readonly data: ThemeDataPalette;
  readonly typography: ThemeTypography;
  readonly form: ThemeForm;
  readonly elevation: ThemeElevation;
  readonly node_ordinal_palette: boolean;
}

export interface ResolvedThemeV2 extends ResolvedFamilyTheme {
  readonly id: string | null;
  readonly appearance: ThemeAppearance;
  readonly canvas: ThemeCanvas;
  readonly family_overrides?: Readonly<Record<string, ResolvedFamilyTheme>>;
}

export type ThemeNotice =
  | {
      readonly kind: "low_contrast";
      readonly foreground: string;
      readonly background: string;
      readonly ratio: number;
      readonly minimum: number;
    }
  | { readonly kind: "contrast_adjusted"; readonly foreground: string; readonly color: string };

export interface ThemeOutcome {
  readonly mode: "mermaid_parity" | "merman_native";
  readonly preset_id?: string;
  readonly appearance?: ThemeAppearance;
  readonly notices: readonly (ThemeNotice | string)[];
}

export interface ThemeResolveOptions {
  readonly appearance?: ThemeAppearance;
  /** Adjust foreground colors in the resolved preview. */
  readonly auto_fix_contrast?: boolean;
}

export interface ThemeResolveError {
  readonly ok: false;
  /** One-based coordinates in the input document, when known. */
  readonly line: number | null;
  readonly column: number | null;
  readonly path: string | null;
  readonly message: string;
}

export type ThemeResolution =
  | { readonly ok: true; readonly resolved: ResolvedThemeV2; readonly notices: readonly ThemeNotice[] }
  | ThemeResolveError;
