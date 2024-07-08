package com.github.kimahriman

import org.sparkproject.connect.grpc.ServerInterceptor
import org.sparkproject.connect.grpc.{Metadata, ServerCall, ServerCallHandler}
import org.sparkproject.connect.grpc.ServerCall.Listener
import org.sparkproject.connect.grpc.Status

class SparkConnectProxyInterceptor extends ServerInterceptor {

  override def interceptCall[ReqT, RespT](
      call: ServerCall[ReqT,RespT],
      metadata: Metadata,
      next: ServerCallHandler[ReqT,RespT]
  ): Listener[ReqT] = {
    val authHeaderValue = metadata.get(Metadata.Key.of("Authorization", Metadata.ASCII_STRING_MARSHALLER));
    println(authHeaderValue)

    if (authHeaderValue == null) {
      val status = Status.UNAUTHENTICATED.withDescription("No auth token provided");
      call.close(status, new Metadata())
    } else if (authHeaderValue != "Bearer deadbeef") {
      val status = Status.UNAUTHENTICATED.withDescription("Invalid auth token");
      call.close(status, new Metadata())
    } else {
      return next.startCall(call, metadata)
    }
    // No-op for close calls
    new Listener[ReqT]() {}
  }
}