// ---------------------------------------------------------------------------
// qt-solid Figma codegen plugin entry point
// ---------------------------------------------------------------------------

import { convertNode, createContext, generateImports } from "./convert.ts"
import { generateVariantComponent } from "./variant-gen.ts"

if (figma.mode === "codegen") {
  figma.codegen.on("generate", async ({ node }) => {
    const outputMode = figma.codegen.preferences.customSettings.output === "fragments"
      ? "fragments" as const
      : "auto" as const

    const results: CodegenResult[] = []

    // If it's a component set, also generate a createVariants definition.
    if (node.type === "COMPONENT_SET") {
      const variantResult = await generateVariantComponent(node)
      if (variantResult) {
        results.push({
          title: `${variantResult.componentName} (createVariants)`,
          language: "TYPESCRIPT",
          code: variantResult.code,
        })
      }
    }

    // Always generate the fragment/component output.
    const ctx = createContext(outputMode)
    const body = await convertNode(node, ctx)
    const imports = generateImports(ctx.imports)
    const code = imports ? `${imports}\n\n${body}` : body

    results.push({
      title: "qt-solid JSX",
      language: "TYPESCRIPT",
      code,
    })

    return results
  })
}
