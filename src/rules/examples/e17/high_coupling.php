<?php

class OrderService extends BaseService implements AuditableService
{
    private UserRepository $users;
    private PaymentGateway $payments;
    private Mailer $mailer;

    public function __construct(Logger $logger, EventBus $events)
    {
    }

    public function refund(Order $order): RefundReceipt
    {
        if ($order instanceof PaidOrder) {
            StripeRefund::create($order);
        }

        try {
            return new RefundReceipt();
        } catch (GatewayException $exception) {
            throw new CannotRefundOrder();
        }
    }
}
