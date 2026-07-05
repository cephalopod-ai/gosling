import { ModeSection } from '../mode/ModeSection';
import { SummarizerSection } from './SummarizerSection';
import { DictationSettings } from '../dictation/DictationSettings';
import { SecurityToggle } from '../security/SecurityToggle';
import { ResponseStylesSection } from '../response_styles/ResponseStylesSection';
import { GoslinghintsSection } from './GoslinghintsSection';
import { SpellcheckToggle } from './SpellcheckToggle';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../ui/card';
import { defineMessages, useIntl } from '../../../i18n';

const i18n = defineMessages({
  modeTitle: {
    id: 'chatSettings.modeTitle',
    defaultMessage: 'Default Mode',
  },
  modeDescription: {
    id: 'chatSettings.modeDescription',
    defaultMessage:
      'Choose the default mode Gosling uses for new sessions. Existing sessions keep their current mode.',
  },
  responseStylesTitle: {
    id: 'chatSettings.responseStylesTitle',
    defaultMessage: 'Response Styles',
  },
  responseStylesDescription: {
    id: 'chatSettings.responseStylesDescription',
    defaultMessage: 'Choose how Gosling should format and style its responses',
  },
  summarizerTitle: {
    id: 'chatSettings.summarizerTitle',
    defaultMessage: 'Context Summarizer',
  },
  summarizerDescription: {
    id: 'chatSettings.summarizerDescription',
    defaultMessage:
      'Use a local LLM to summarize older context and extract durable facts to memory. Falls back to deterministic truncation whenever the endpoint is unavailable.',
  },
});

export default function ChatSettingsSection() {
  const intl = useIntl();

  return (
    <div className="space-y-4 pr-4 pb-8 mt-1">
      <Card className="pb-2 rounded-lg">
        <CardHeader className="pb-0">
          <CardTitle className="">{intl.formatMessage(i18n.modeTitle)}</CardTitle>
          <CardDescription>{intl.formatMessage(i18n.modeDescription)}</CardDescription>
        </CardHeader>
        <CardContent className="px-2">
          <ModeSection />
        </CardContent>
      </Card>

      <Card className="pb-2 rounded-lg">
        <CardContent className="px-2">
          <GoslinghintsSection />
        </CardContent>
      </Card>

      <Card className="pb-2 rounded-lg">
        <CardContent className="px-2">
          <DictationSettings />
          <SpellcheckToggle />
        </CardContent>
      </Card>

      <Card className="pb-2 rounded-lg">
        <CardHeader className="pb-0">
          <CardTitle className="">{intl.formatMessage(i18n.responseStylesTitle)}</CardTitle>
          <CardDescription>{intl.formatMessage(i18n.responseStylesDescription)}</CardDescription>
        </CardHeader>
        <CardContent className="px-2">
          <ResponseStylesSection />
        </CardContent>
      </Card>

      <Card className="pb-2 rounded-lg">
        <CardHeader className="pb-0">
          <CardTitle className="">{intl.formatMessage(i18n.summarizerTitle)}</CardTitle>
          <CardDescription>{intl.formatMessage(i18n.summarizerDescription)}</CardDescription>
        </CardHeader>
        <CardContent className="px-2">
          <SummarizerSection />
        </CardContent>
      </Card>

      <Card className="pb-2 rounded-lg">
        <CardContent className="px-2">
          <SecurityToggle />
        </CardContent>
      </Card>
    </div>
  );
}
