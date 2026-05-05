<?php

class ReportBuilder
{
    private array $items = [];
    private ?ReportFormatter $formatter = null;

    public function withFormatter(ReportFormatter|ReportFormatter $formatter): self
    {
        $this->formatter = $formatter;

        return $this;
    }

    public function merge(static|self $builder): ReportBuilder
    {
        return $this;
    }

    public function build(): Report
    {
        if ($this instanceof ReportBuilder) {
            return Report::from($this->items);
        }

        return new Report();
    }
}
