// Run with bun and rustfmt on PATH: bun scripts/export-icons.ts [--check]
// Dependencies are installed in /tmp and removed on success or failure.
// Offline: set HUGEICONS_PACKAGE to a cached core-free-icons package directory.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  readFileSync,
  writeFileSync,
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  cpSync,
} from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const install = mkdtempSync("/tmp/lp-hugeicons-");
try {
  // Bun resolves local packages' dev dependencies too; stage only their runtime manifest.
  if (process.env.HUGEICONS_PACKAGE) {
    cpSync(process.env.HUGEICONS_PACKAGE, `${install}/package`, {
      recursive: true,
    });
    const manifestPath = `${install}/package/package.json`;
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    delete manifest.devDependencies;
    writeFileSync(manifestPath, JSON.stringify(manifest));
  }
  writeFileSync(
    `${install}/package.json`,
    JSON.stringify({
      private: true,
      dependencies: {
        "@hugeicons/core-free-icons": process.env.HUGEICONS_PACKAGE
          ? "file:./package"
          : "4.3.5",
      },
    }),
  );
  execFileSync(
    process.execPath,
    ["install", "--production", "--ignore-scripts", "--no-cache"],
    {
      cwd: install,
      env: {
        ...process.env,
        TMPDIR: install,
        BUN_INSTALL_CACHE_DIR: `${install}/cache`,
      },
      stdio: "pipe",
    },
  );
  const require = createRequire(`${install}/package.json`);
  const hugeicons = require("@hugeicons/core-free-icons");
  const version = require("@hugeicons/core-free-icons/package.json").version;
  assert.equal(version, "4.3.5", "Expected core-free-icons 4.3.5");

  const iconNames = {
    IconGraph: "NeuralNetworkIcon",
    IconClassroom: "Mortarboard01Icon",
    IconDigest: "Layers01Icon",
    IconSparkles: "SparklesIcon",
    IconCheck: "Tick02Icon",
    IconX: "Cancel01Icon",
    IconChevronRight: "ArrowRight01Icon",
    IconChevronDown: "ArrowDown01Icon",
    IconChevronLeft: "ArrowLeft01Icon",
    IconChevronUp: "ArrowUp01Icon",
    IconSearch: "Search01Icon",
    IconSettings: "Settings01Icon",
    IconTerminal: "CommandLineIcon",
    IconBook: "BookOpen01Icon",
    IconBrain: "BrainIcon",
    IconRefresh: "RefreshIcon",
    IconPlay: "PlayIcon",
    IconShield: "Shield01Icon",
    IconPlus: "Add01Icon",
    IconTrash: "Delete01Icon",
    IconCopy: "Copy01Icon",
    IconCode: "CodeIcon",
    IconCommand: "CommandIcon",
    IconUsers: "UserGroupIcon",
    IconZoomIn: "ZoomInIcon",
    IconZoomOut: "ZoomOutIcon",
    IconFilter: "FilterIcon",
    IconInfo: "InformationCircleIcon",
  } as const;

  const target = `${root}crates/lp-ui/src/icons.rs`;
  const original = readFileSync(target, "utf8");
  const components = [
    ...original.matchAll(/pub fn (Icon\w+)\(props: IconProps\) -> Element/g),
  ].map((match) => match[1]);
  assert.deepEqual(
    components.sort(),
    Object.keys(iconNames).sort(),
    "Update iconNames for every public icon",
  );

  // Retain the shared props/renderer and each component's public signature and test ID.
  const firstComponent = original.indexOf("#[component]");
  const prefixEnd = original.lastIndexOf("\n///", firstComponent);
  let output = original
    .slice(0, prefixEnd)
    .replace("Lucide SVG", "Hugeicons SVG")
    .replace('stroke_width: "2"', 'stroke_width: "1.5"');
  const tests: string[] = [];
  const attributes: Record<string, string> = {
    d: "d",
    cx: "cx",
    cy: "cy",
    r: "r",
    fill: "fill",
    stroke: "stroke",
    strokeWidth: "stroke_width",
    strokeLinecap: "stroke_linecap",
    strokeLinejoin: "stroke_linejoin",
    fillRule: "fill_rule",
    clipRule: "clip_rule",
    opacity: "opacity",
    strokeOpacity: "stroke_opacity",
    fillOpacity: "fill_opacity",
  };

  for (const [component, name] of Object.entries(iconNames)) {
    const nodes = hugeicons[name] as [
      string,
      Record<string, string | number>,
    ][];
    assert(
      Array.isArray(nodes) && nodes.length > 0,
      `Missing Hugeicons export: ${name}`,
    );
    assert(
      nodes.some(([tag, attrs]) => tag === "path" && attrs.d),
      `${name} has no path`,
    );
    const existing = original.slice(original.indexOf(`pub fn ${component}(`));
    const testId = existing.match(/"(icon-[a-z-]+)"/)?.[1];
    assert(testId, `Missing existing test ID: ${component}`);
    const paths = nodes
      .map(([tag, attrs]) => {
        assert(
          ["path", "circle"].includes(tag),
          `Unsupported SVG node: ${tag}`,
        );
        const props = Object.entries(attrs)
          .filter(([key]) => key !== "key")
          .map(([key, value]) => {
            assert(attributes[key], `Unsupported SVG attribute: ${key}`);
            return `${attributes[key]}: ${JSON.stringify(String(value))}`;
          });
        return `            ${tag} { ${props.join(", ")} }`;
      })
      .join("\n");
    output += `\n/// Hugeicons \`${name}\` (core-free-icons ${version}; MIT, see assets/hugeicons-LICENSE.md).
#[component]
pub fn ${component}(props: IconProps) -> Element {
    render_svg(
        &props,
        "${testId}",
        rsx! {
${paths}
        },
    )
}
`;
    tests.push(`    #[test]
    fn ${testId.replaceAll("-", "_")}_renders_svg_with_paths() {
        let html = dioxus::ssr::render_element(rsx! { ${component} {} });
        assert!(html.starts_with("<svg"));
        assert!(html.ends_with("</svg>"));
        assert!(html.contains("<path "));
        assert_eq!(html.matches("<path ").count(), ${nodes.filter(([tag]) => tag === "path").length});
        assert!(html.contains("data-testid=\\"${testId}\\""));
        assert!(html.contains("aria-hidden=\\"true\\""));
    }
`);
  }

  output += `
#[cfg(test)]
mod tests {
    use super::*;

${tests.join("\n")}
    #[test]
    fn icon_props_preserve_accessibility_size_and_attributes() {
        let html = dioxus::ssr::render_element(rsx! {
            IconGraph {
                size: 20,
                class: "graph-icon",
                aria_label: "Knowledge Graph",
                test_id: "custom-graph",
                title: "Graph",
            }
        });
        for attribute in [
            "width=\\"20\\"", "height=\\"20\\"", "class=\\"graph-icon\\"",
            "aria-label=\\"Knowledge Graph\\"", "role=\\"img\\"",
            "data-testid=\\"custom-graph\\"", "title=\\"Graph\\"",
            "stroke=\\"currentColor\\"",
        ] {
            assert!(html.contains(attribute), "Missing {attribute}");
        }
        assert!(!html.contains("aria-hidden="));
    }
}
`;

  output = execFileSync("rustfmt", ["--edition", "2024"], {
    input: output,
    encoding: "utf8",
  });
  const licenseSource = require
    .resolve("@hugeicons/core-free-icons/package.json")
    .replace(/package\.json$/, "LICENSE.md");
  const licenseTarget = `${root}crates/lp-ui/assets/hugeicons-LICENSE.md`;
  if (process.argv.includes("--check")) {
    assert.equal(
      original,
      output,
      "icons.rs differs from installed Hugeicons; rerun exporter",
    );
    assert.equal(
      readFileSync(licenseTarget, "utf8"),
      readFileSync(licenseSource, "utf8"),
    );
  } else {
    mkdirSync(`${root}crates/lp-ui/assets`, { recursive: true });
    writeFileSync(target, output);
    copyFileSync(licenseSource, licenseTarget);
  }
  console.log(
    `${target}: ${components.length} Hugeicons components, 0 unmatched`,
  );
} finally {
  rmSync(install, { recursive: true, force: true });
}
