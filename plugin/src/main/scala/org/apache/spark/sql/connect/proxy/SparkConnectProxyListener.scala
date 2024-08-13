package org.apache.spark.sql.connect.proxy

import java.net.http.HttpRequest
import java.net.URI
import java.time.Duration

import java.time.temporal.ChronoUnit.SECONDS

import org.apache.spark.SparkContext
import org.apache.spark.internal.Logging
import org.apache.spark.scheduler.SparkListener
import org.apache.spark.scheduler.SparkListenerEvent
import org.apache.spark.sql.connect.service.SparkConnectService
import org.apache.spark.sql.connect.service.SparkListenerConnectServiceStarted
import java.net.http.HttpClient
import java.net.http.HttpResponse.BodyHandlers
import org.apache.spark.SparkException
import org.apache.spark.SparkConf

class SparkConnectProxyListener(conf: SparkConf) extends SparkListener with Logging {

  val callbackAddr = {
    val addr = conf.get(Config.SPARK_CONNECT_PROXY_CALLBACK)
    assert(addr.isDefined, "No callback address provided")
    addr.get
  }

  val token = {
    val token = conf.get(Config.SPARK_CONNECT_PROXY_TOKEN)
    assert(token.nonEmpty, "No token provided, can't authenticate requests")
    token.get
  }

  override def onOtherEvent(event: SparkListenerEvent): Unit = {
    event match {
      case SparkListenerConnectServiceStarted(hostAddress, bindingPort, _, _) =>
        val connectUri = s"$hostAddress:$bindingPort"

        logInfo(s"Connect service started on $connectUri")
        
        val request = HttpRequest.newBuilder()
          .uri(URI.create(s"$callbackAddr/callback"))
          .timeout(Duration.of(10, SECONDS))
          .setHeader("Authorization", s"Bearer $token")
          .setHeader("Content-type", "application/json")
          .POST(HttpRequest.BodyPublishers.ofString(s"{\"address\": \"$connectUri\"}"))
          .build()

        logInfo(s"Sending callback info to ${request.uri()}")

        val client = HttpClient.newHttpClient()

        try {
          val response = client.send(request, BodyHandlers.discarding())

          if (response.statusCode() != 200) {
            logError(s"Bad status code returned from proxy server: ${response.statusCode()}")
            SparkConnectService.stop()
          }
        }
        catch {
          case e: Throwable => {
            logError(s"Failed to send connect address to callback", e)
            SparkConnectService.stop()
          }
        }
      case _ => ()
    }
  }
}
