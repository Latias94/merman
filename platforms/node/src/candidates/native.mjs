import { loadNativeBinding, wrapNativeBinding } from "../native-loader.mjs";

export async function loadNativeTransport(optionsJson, loaderOptions = {}, iconPacksJson) {
  return wrapNativeBinding(loadNativeBinding(loaderOptions), optionsJson, iconPacksJson);
}
