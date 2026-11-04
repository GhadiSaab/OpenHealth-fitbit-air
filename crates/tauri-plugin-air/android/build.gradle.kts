plugins { id("com.android.library"); id("org.jetbrains.kotlin.android") }
android { namespace = "app.airspace.air"; compileSdk = 37
 defaultConfig { minSdk = 26; consumerProguardFiles("consumer-rules.pro") }
 compileOptions { sourceCompatibility = JavaVersion.VERSION_1_8; targetCompatibility = JavaVersion.VERSION_1_8 }
}
kotlin { compilerOptions { jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_1_8 } }
dependencies {
 implementation(project(":tauri-android"))
 implementation("androidx.activity:activity-ktx:1.10.1")
 implementation("com.fasterxml.jackson.core:jackson-databind:2.15.3")
 implementation("org.bouncycastle:bctls-jdk18on:1.86.2")
 implementation("org.bouncycastle:bcprov-jdk18on:1.86")
 implementation("org.bouncycastle:bcutil-jdk18on:1.86")
}
