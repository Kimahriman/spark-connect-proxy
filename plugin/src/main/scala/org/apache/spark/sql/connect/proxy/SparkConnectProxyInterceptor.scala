package org.apache.spark.sql.connect.proxy

import org.apache.spark.SparkContext
import org.sparkproject.connect.grpc.ServerInterceptor
import org.sparkproject.connect.grpc.{Metadata, ServerCall, ServerCallHandler}
import org.sparkproject.connect.grpc.ServerCall.Listener
import org.sparkproject.connect.grpc.Status

class SparkConnectProxyInterceptor extends ServerInterceptor {

  val sparkContext = SparkContext.getActive.get

  val token = {
    val token = sparkContext.getConf.get(Config.SPARK_CONNECT_PROXY_TOKEN)
    assert(token.nonEmpty, "No token provided, can't authenticate requests")
    token.get
  }

  override def interceptCall[ReqT, RespT](
      call: ServerCall[ReqT,RespT],
      metadata: Metadata,
      next: ServerCallHandler[ReqT,RespT]
  ): Listener[ReqT] = {
    val authHeaderValue = metadata.get(Metadata.Key.of("Authorization", Metadata.ASCII_STRING_MARSHALLER));

    if (authHeaderValue == null) {
      val status = Status.UNAUTHENTICATED.withDescription("No auth token provided");
      call.close(status, new Metadata())
    } else if (authHeaderValue != s"Bearer $token") {
      val status = Status.UNAUTHENTICATED.withDescription("Invalid auth token");
      call.close(status, new Metadata())
    } else {
      return next.startCall(call, metadata)
    }
    // No-op for close calls
    new Listener[ReqT]() {}
  }
}
