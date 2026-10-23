fn main() {
    // دانلودمنیجر به دسترسی ادمین نیاز ندارد؛ سطح اجرا asInvoker است.
    #[cfg(windows)]
    {
        // نکته‌ی حیاتی (درس LagSlice): این مانیفست جایگزین مانیفست پیش‌فرض Tauri
        // می‌شود؛ پس dependency مربوط به Common-Controls نسخه‌ی ۶ هم باید داخلش
        // باشد، وگرنه ویندوز comctl32 قدیمی (v5.82) را لود می‌کند و برنامه موقع
        // اجرا با خطای «TaskDialogIndirect could not be located» بالا نمی‌آید.
        let manifest = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}" />
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}" />
      <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}" />
      <supportedOS Id="{e2011457-1546-43c5-a5fe-008deee3d3f0}" />
    </application>
  </compatibility>
</assembly>"#;
        let windows_attrs = tauri_build::WindowsAttributes::new().app_manifest(manifest);
        let attrs = tauri_build::Attributes::new().windows_attributes(windows_attrs);
        tauri_build::try_build(attrs).expect("failed to run tauri build");
    }
    #[cfg(not(windows))]
    tauri_build::build();
}
