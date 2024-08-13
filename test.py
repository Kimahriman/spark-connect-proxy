import grpc
from pyspark.sql.connect.session import SparkSession
from pyspark.sql.connect.client.core import DefaultChannelBuilder

class TokenInterceptor(grpc.UnaryUnaryClientInterceptor, grpc.UnaryStreamClientInterceptor, grpc.StreamUnaryClientInterceptor, grpc.StreamStreamClientInterceptor):
    def _intercept(self, continuation, client_call_details, request):
        if not any((key == 'authorization' for key, _ in client_call_details.metadata)):
            client_call_details.metadata.append(("authorization", "Bearer 497532ba-406b-4e35-bb77-75eebbda962b"))
        print(client_call_details.metadata, client_call_details.credentials)
        return continuation(client_call_details, request)

    def intercept_unary_unary(self, continuation, client_call_details, request):
        return self._intercept(continuation, client_call_details, request)

    def intercept_unary_stream(self, continuation, client_call_details, request):
        return self._intercept(continuation, client_call_details, request)
    
    def intercept_stream_unary(self, continuation, client_call_details, request_iterator):
        return self._intercept(continuation, client_call_details, request_iterator)
    
    def intercept_stream_stream(self, continuation, client_call_details, request_iterator):
        return self._intercept(continuation, client_call_details, request_iterator)

channel_builder = DefaultChannelBuilder('sc://localhost:8100/')
channel_builder.add_interceptor(TokenInterceptor())

spark = SparkSession.Builder().channelBuilder(channel_builder).getOrCreate()
# spark = SparkSession.Builder().remote('sc://localhost:8100/;token=deadbeef').getOrCreate()
spark.range(5).show()
spark.stop()