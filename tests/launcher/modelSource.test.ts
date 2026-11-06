import {expect, test} from "bun:test";
import {hasDlssnrConfiguration, modelSourcePaths} from "../../src/services/modConfiguration";
import {isMissingModel} from "../../src/services/installationMessages";
const config = (dll_version:string) => ({backend:"AMDNR",dll_version,shortcut_key:"Insert"});

test("configured model and installation payload agree for legacy names and supported archives", () => {
  for (const file of ["/models/dlssnr.bin", "/models/Extracted Model.BIN", "/models/nvngx_dlssnr.DLL", "/models/NVIDIA.ZIP"]) {
    expect(hasDlssnrConfiguration(config(file))).toBe(true);
    const source = modelSourcePaths(config(` ${file} `));
    expect(source.binPath ?? source.dllPath).toBe(file);
    expect(Boolean(source.binPath) !== Boolean(source.dllPath)).toBe(true);
  }
  for (const file of ["", "0.5.1", "/models/unsupported.txt"]) {
    expect(hasDlssnrConfiguration(config(file))).toBe(false);
    expect(modelSourcePaths(config(file))).toEqual({binPath:null,dllPath:null});
  }
});

test("backend missing-model log opens recovery without misclassifying a damaged model", () => {
  expect(isMissingModel('Script Error (code: Some(1)):\nThe package has no model file: point --dll at nvngx_dlssnr.dll (310.9.16 - both models - or 310.8.0) or its zip.')).toBe(true);
  expect(isMissingModel('MODEL_BIN_NOT_FOUND')).toBe(true);
  expect(isMissingModel('The extracted model has the wrong checksum; nothing was installed.')).toBe(false);
  expect(isMissingModel('Permission denied while copying the model')).toBe(false);
});
