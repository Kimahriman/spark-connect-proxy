package org.apache.spark.sql.connect.proxy

import org.apache.spark.internal.config.ConfigBuilder
import java.util.concurrent.TimeUnit

object Config {
  // The auth token that must be used by the client to connect
  val SPARK_CONNECT_PROXY_TOKEN =
    ConfigBuilder("spark.connect.proxy.token")
      .stringConf
      .createOptional

  // The auth token that must be used by the client to connect
  val SPARK_CONNECT_PROXY_CALLBACK =
    ConfigBuilder("spark.connect.proxy.callback")
      .stringConf
      .createOptional

  // How long after no activity do we kill the session
  val SPARK_CONNECT_PROXY_IDLE_TIMEOUT =
    ConfigBuilder("spark.connect.proxy.idle.timeout")
      .timeConf(TimeUnit.SECONDS)
      .createOptional
}
