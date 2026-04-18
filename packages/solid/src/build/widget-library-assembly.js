export const QT_SOLID_RUNTIME_ID = "\0qt-solid:runtime"
export const QT_SOLID_COMPILER_RT_ID = "\0qt-solid:compiler-rt"
export const QT_SOLID_REGISTRATION_ID = "\0qt-solid:runtime-registration"
export const DEFAULT_WIDGET_LIBRARIES = ["@qt-solid/core-widgets"]

export function resolveWidgetLibraryEntrySpecifier(input) {
  return input.endsWith("/widget-library") ? input : `${input}/widget-library`
}

export function normalizeWidgetLibraries(input) {
  const libraries = input ?? DEFAULT_WIDGET_LIBRARIES
  return libraries.map(resolveWidgetLibraryEntrySpecifier)
}

export function renderQtSolidRegistrationModule(widgetLibraries) {
  const imports = [
    `import { registerQtWidgetLibraryEntry } from ${JSON.stringify("@qt-solid/core/widget-library")}`,
  ]
  const registrations = []

  for (const [index, widgetLibrary] of widgetLibraries.entries()) {
    const bindingName = `qtWidgetLibraryEntry${String(index)}`
    imports.push(
      `import { qtWidgetLibraryEntry as ${bindingName} } from ${JSON.stringify(widgetLibrary)}`,
    )
    registrations.push(`registerQtWidgetLibraryEntry(${bindingName}, { default: ${index === 0 ? "true" : "false"} })`)
  }

  return [
    ...imports,
    "",
    ...registrations,
  ].join("\n")
}

export function renderQtSolidRuntimeModule(runtimeEntry, registrationId = QT_SOLID_REGISTRATION_ID) {
  return [
    `import ${JSON.stringify(registrationId)}`,
    "",
    `export * from ${JSON.stringify(runtimeEntry)}`,
  ].join("\n")
}
