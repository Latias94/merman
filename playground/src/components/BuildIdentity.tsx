import { useTranslation } from "react-i18next";

/** Deployment identity is separate from the WASM package's semantic version. */
export function BuildIdentity() {
  const { t } = useTranslation();
  const build = __PLAYGROUND_BUILD__;
  const label = [
    t(`build.${build.channel}`),
    build.commit?.slice(0, 9),
    build.dirty ? t("build.modified") : null,
  ].filter(Boolean).join(" · ");
  const description = [
    t("build.description"),
    build.ref,
    build.commit,
  ].filter(Boolean).join("\n");
  const className = "shrink-0 whitespace-nowrap text-xs text-muted-foreground";
  return build.commit && build.repository ? (
    <a
      className={`${className} underline decoration-dotted underline-offset-4 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring`}
      href={`https://github.com/${build.repository}/commit/${build.commit}`}
      target="_blank"
      rel="noopener noreferrer"
      title={description}
      aria-label={`${label}. ${t("build.viewCommit")}`}
      data-merman-build={build.channel}
    >
      {label}
    </a>
  ) : (
    <span className={className} title={description} data-merman-build={build.channel}>
      {label}
    </span>
  );
}
