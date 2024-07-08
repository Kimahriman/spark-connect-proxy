ThisBuild / organization := "com.github.kimahriman"
ThisBuild / version := "0.1.0-SNAPSHOT"
ThisBuild / scalaVersion := "2.13.14"
// ThisBuild / scalaVersion := "2.12.18"

lazy val plugin = (project in file("plugin"))
  .settings(
    name := "spark-connect-proxy-plugin",
    libraryDependencies ++= Seq(
      "org.apache.spark" % "spark-connect_2.13" % "4.0.0-preview1" % Provided,
    )
  )

autoScalaLibrary := false
crossPaths := false
publishArtifact := false  // Don't release the root project
publish / skip := true